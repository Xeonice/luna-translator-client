/* Pending C35 design supplement. No production imports, native calls, network,
   Hook, model, or permission APIs. All inputs below are labelled design fixtures. */
(() => {
  'use strict';
  const $ = id => document.getElementById(id);
  const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const icon = name => window.LunaIcons.svg(name);
  const params = new URLSearchParams(location.search);
  const epoch = Date.UTC(2026, 9, 18, 10, 24, 8);
  const game = window.LunaDesignData.games.find(item => item.id === 'sakura');
  const title = game.title;
  const longTitle = 'サクラノ詩 Re:Graphic 10th Anniversary edition ― 特別限定パッケージ・追加ストーリー収録版（日本語）';
  const states = {
    live: {label:'实时预览',guidance:'自动更新所选窗口的画面。',mode:'available'},
    paused: {label:'已暂停',guidance:'已保留最后画面；游戏取文继续。',mode:'available'},
    stale: {label:'画面已过期',guidance:'当前画面暂不可用，保留最后成功画面与原采集时间。',mode:null},
    starting: {label:'正在开启',empty:'等待第一张有效画面',guidance:'等待有效画面；游戏取文继续。',mode:'starting'},
    permission_denied: {label:'需要授权',empty:'需要屏幕录制权限',guidance:'允许 Luna 录屏后，核验有效目标即可恢复。',recovery:'授权说明…',mode:'permission'},
    permission_revoked: {label:'权限已撤销',empty:'需要重新允许屏幕录制',guidance:'恢复权限后重新检查；游戏取文继续。',recovery:'授权说明…',mode:'permission'},
    unavailable: {label:'暂不可用',empty:'窗口当前不可采集',guidance:'恢复游戏窗口，或重新选择窗口。',recovery:'选择窗口…',mode:'unavailable'},
    target_closed: {label:'目标窗口已关闭',empty:'预览已停止',guidance:'重新打开游戏后，再选择运行窗口。',recovery:'重新连接…',mode:'exited'},
    identity_error: {label:'身份待核对',empty:'窗口身份已变化',guidance:'重新核对游戏窗口，避免显示错窗。',recovery:'重新核对…',mode:'identity_error'}
  };
  const allowed = (id, value, fallback) => [...$(id).options].some(option => option.value === value) ? value : fallback;
  const state = {
    state: allowed('pc-state', params.get('state'), 'live'), width: allowed('pc-width', params.get('width'), '282'),
    theme: allowed('pc-theme', params.get('theme'), 'dark'), zoom: allowed('pc-zoom', params.get('zoom'), '1'),
    title: allowed('pc-title', params.get('title'), 'normal'), owner: allowed('pc-owner', params.get('owner'), 'candidate'), origin: allowed('pc-origin', params.get('origin'), 'candidate'),
    trust: allowed('pc-trust', params.get('trust'), 'selected'), capability: allowed('pc-capability', params.get('capability'), 'granted'),
    frameIndex: 0, sampleClockMs: epoch,
    lastSuccessfulAtMs: ['live','paused','stale','permission_revoked','unavailable','target_closed'].includes(allowed('pc-state', params.get('state'), 'live')) ? epoch : null
  };
  let focusedTrigger = null, copyPending = false, openCycle = 0;
  const displayTime = ms => ms == null ? '尚无成功画面' : new Date(ms).toLocaleTimeString('zh-CN', {hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3});
  const fullTime = ms => ms == null ? '未提供' : new Date(ms).toISOString() + '（' + ms + ' ms）';
  const currentTitle = () => state.title === 'long' ? longTitle : title;
  const scene = () => game.lines[state.frameIndex % game.lines.length].scene;
  const hasFrame = () => state.lastSuccessfulAtMs !== null && ['live','paused','stale'].includes(state.state);
  const historyTime = () => state.lastSuccessfulAtMs;
  const displayState = () => historyTime() === null && ['live','stale'].includes(state.state) ? 'starting' : state.state;
  function presentation() {
    if (historyTime() === null && state.state === 'paused') return {...states.paused,empty:'尚无成功画面',guidance:'当前预览已暂停；尚无成功采集记录。'};
    if (historyTime() === null && ['live','stale'].includes(state.state)) return {...states.starting,guidance:'尚未收到有效帧，不能标为实时或过期画面。'};
    return states[state.state];
  }
  const projectionPermission = () => ['permission_denied','permission_revoked'].includes(state.state) ? '拒绝' : ['live','paused','stale'].includes(state.state) ? '允许（设计条件）' : '未获得结果';
  const capabilityPermission = () => ({granted:'允许',denied:'拒绝',unknown:'未获得结果'}[state.capability]);
  function identity() {
    if (state.state === 'identity_error') return {label:'身份未核对',symbol:'alert'};
    if (state.trust === 'verified') return {label:'身份已核对',symbol:'check'};
    if (state.trust === 'unknown' || state.origin === 'game_record') return {label:'身份待核对',symbol:'window'};
    return {label:'已选择目标',symbol:'check'};
  }
  function rawFactRows() {
    const kind = state.origin === 'candidate' ? '候选发现（设计样例）' : '游戏资料（可编辑，非本次运行核验）';
    const missing = '未提供';
    return [
      ['采集与状态', [
        ['当前状态',presentation().label], ['状态说明',presentation().guidance],
        ['当前显示画面时间',hasFrame() ? fullTime(historyTime()) : '无可显示的有效画面'],
        ['最近成功时间（历史事实）',fullTime(historyTime())],
        ['历史事实范围','此目标上一次成功采集的记录；恢复、权限撤销和关闭不抹去已知历史时间。历史时间本身不证明当前有权限、目标有效或画面可复用。'],
        ['画面来源',hasFrame() ? '当前订阅接收的有效目标画面；非实时状态保留最后成功画面。' : '当前未显示可复用画面。'],
        ['会话归属',state.owner==='candidate'?'候选核对 · 未确认接入':'当前会话常驻 · 跨页面继续'], ['目标事实来源',kind]
      ]],
      ['权限与身份', [
        ['采集投影中的权限',projectionPermission()],
        ['最近能力检查的权限',capabilityPermission()],
        ['权限说明','两项分别来自采集投影与能力检查，不能用一次允许结果覆盖采集器的拒绝或撤销。'],
        ['完整游戏名称',currentTitle()],
        [state.origin === 'candidate' ? '候选窗口标题' : '资料中的窗口名称',currentTitle()],
        ['环境 / Bottle',game.bottle],
        [state.origin === 'candidate' ? '候选程序完整路径' : '资料中的程序路径',game.path],
        ['Windows 本次运行 PID / 创建时间',missing], ['宿主 PID / 创建时间',missing],
        ['原生窗口 ID / HWND',missing], ['目标身份摘要 / hash',missing],
        ['核验依据',state.trust === 'verified' && state.state !== 'identity_error' ? '设计条件：已取得完整目标核验记录。此页面未连接真实系统，不能作为该记录。' : '仅有目标选择或资料不能证明本次运行身份；需独立完整核验记录。']
      ]],
      ['诊断原文', [
        ['阶段',missing], ['请求 / Native 操作',missing], ['Native 状态',missing], ['API',missing],
        ['原始原因',missing], ['原始诊断全文','本设计样例未提供 Native 返回。正式产品只显示本轮实际返回的阶段、操作、错误码和原因，不填固定假值。']
      ]],
      ['隐私与保存', [['规则','只预览已明确选择并核对的游戏窗口，不采音频、不上传。持续画面不录制、不连续保存；封面独立按既定授权处理。刷新、详情、暂停不激活游戏，不改变游戏窗口尺寸、DPI 或图形设置。']]]
    ];
  }
  function renderDetailsBody() {
    $('pc-details-body').innerHTML = rawFactRows().map(([heading,rows]) => '<section><h3>'+escape(heading)+'</h3><dl>'+rows.map(([label,value])=>'<div><dt>'+escape(label)+'</dt><dd>'+escape(value)+'</dd></div>').join('')+'</dl></section>').join('');
  }
  function renderProposal() {
    const fact = presentation(), currentIdentity = identity(), retained = state.state !== 'live';
    const image = hasFrame() ? '<img src="'+escape(scene())+'" alt="'+escape(currentTitle())+' · '+(retained?'最后成功画面':'当前画面')+'"'+(retained?' class="pc-last-frame"':'')+'>'+(retained?'<span class="pc-frame-kind">最后画面</span>':'') : '<div class="pc-empty">'+icon(state.state==='identity_error'?'alert':'window')+'<strong>'+escape(fact.empty)+'</strong></div>';
    const pauseEnabled = ['live','paused','stale'].includes(state.state);
    $('pc-proposal-host').innerHTML = '<section class="pc-preview" aria-label="运行窗口预览" data-state="'+displayState()+'"><div class="pc-preview-layout"><header class="pc-preview-heading"><strong title="'+escape(currentTitle())+'">'+escape(currentTitle())+'</strong><button class="pc-info" id="pc-open-details" type="button" aria-label="预览详情" aria-haspopup="dialog" aria-controls="pc-details" aria-expanded="'+$('pc-details').open+'" title="预览详情">'+icon('ellipsis')+'</button></header><div class="pc-preview-image">'+image+'<span class="pc-state-pill">'+escape(fact.label)+'</span></div><div class="pc-preview-meta"><div><span class="pc-preview-time"><span>'+(hasFrame()&&retained?'原采集时间':'采集时间')+'</span><time datetime="'+(hasFrame()?new Date(historyTime()).toISOString():'')+'">'+(hasFrame()?displayTime(historyTime()):'尚无成功画面')+'</time></span><small class="pc-identity">'+icon(currentIdentity.symbol)+'运行窗口预览 · '+escape(currentIdentity.label)+'</small></div><p class="pc-guidance">'+escape(fact.guidance)+'</p></div><footer class="pc-preview-actions"><button id="pc-refresh" type="button"'+(state.state==='target_closed'?' disabled':'')+'>'+icon('history')+'刷新</button><button id="pc-pause" type="button" aria-pressed="'+(state.state==='paused')+'"'+(!pauseEnabled?' disabled':'')+'>'+icon(state.state==='paused'?'play':'pause')+(state.state==='paused'?'继续':'暂停')+'</button>'+(fact.recovery?'<button class="pc-recover" id="pc-recover" type="button">'+escape(fact.recovery)+'</button>':'')+'</footer></div></section>';
    $('pc-open-details').addEventListener('click', openDetails);
    $('pc-refresh').addEventListener('click', () => message('设计演示：发出重新检查意图；尚未收到新帧，原采集时间不变。'));
    $('pc-pause').addEventListener('click', () => { state.state = state.state === 'paused' ? 'starting' : 'paused'; render(); message(state.state === 'paused' ? '设计演示：暂停保留最后一帧和原时间，Hook 取文保持独立。' : '设计演示：已发出继续意图，等待新的有效帧；不会直接把缓存画面标为实时。'); });
    $('pc-recover')?.addEventListener('click', () => message('设计演示：'+(['permission_denied','permission_revoked'].includes(state.state)?'显示授权说明；不会在浏览器请求或更改系统录屏权限。':'打开统一目标核对流程；不会自动重注入 Hook 或更改游戏窗口。')));
    if ($('pc-details').open) { renderDetailsBody(); placeDetails(); }
  }
  const originalTarget = {recordId:game.id,hostWindowId:'design-window-101',title,environment:'CrossOver',bottle:game.bottle,exe:game.exe,targetPath:game.path};
  const app = {state:{session:{gameId:game.id,status:'connected',runtimeTarget:originalTarget},current:{scene:scene()},windows:{main:true},settings:{general:{},preferences:{}}}, game:()=>game, render:renderOriginal, openWindow:()=>{}};
  window.LunaDesign = app;
  window.LunaLibrary = {allGames:()=>[game],windowChoices:()=>[originalTarget]};
  window.LunaReviewGaps.setup(app);
  function withOriginalClock(fn) {
    const NativeDate=window.Date;
    window.Date=class extends NativeDate {constructor(...args){super(...(args.length?args:[historyTime() ?? state.sampleClockMs]));}static now(){return historyTime() ?? state.sampleClockMs;}};
    try{return fn();}finally{window.Date=NativeDate;}
  }
  function renderOriginal() {
    if (state.state === 'stale') {
      $('pc-original-host').innerHTML='<div class="pc-original-absent"><strong>原稿缺少“画面已过期”状态</strong><p>此处没有编造替代参考，也不使用“暂不可用”冒充。拟稿在右侧明确展示最后画面与原时间。</p></div>';
      return;
    }
    app.state.session.status=state.state==='target_closed'?'exited':'connected';
    app.state.current.scene=scene();originalTarget.title=currentTitle();
    const hasHistory=historyTime() !== null;
    window.LunaReviewGaps.previews.set(originalTarget.hostWindowId,{enabled:true,paused:state.state==='paused',scene:hasHistory?scene():null,frameSource:hasHistory?scene():null,updated:hasHistory?displayTime(historyTime()):null,updatedIso:hasHistory?new Date(historyTime()).toISOString():null});
    $('pc-original-host').innerHTML=withOriginalClock(()=>window.LunaReviewGaps.previewHTML(app,originalTarget,{selection:state.owner==='candidate'}));
  }
  function setOriginalMode() {
    if (state.state==='stale')return renderOriginal();
    $('review-preview').value=states[state.state].mode;
    withOriginalClock(()=>$('review-preview').dispatchEvent(new Event('change',{bubbles:true})));
  }
  function message(text) { $('pc-review-message').textContent=text; }
  function updateReviewNote() {
    const base='所有画面和事实均为设计样例；工具变化不代表真实权限、采集、Hook 或产品验收通过。';
    message((state.trust==='verified' ? base+'“身份已核对”仅示意完整核验记录成立后的样子。' : base)+(displayState()!==state.state?' 当前没有成功帧事实，拟稿保持等待，不把所选条件标成实时。':''));
    $('pc-original-note').textContent=state.state==='permission_revoked' ? '原稿没有独立撤销状态；这里如实展示原 permission 模式在已有旧帧时的表现。' : state.state==='stale' ? '未生成旧稿不存在的参考状态。' : '仅展示该源稿在给定设计输入下的原始呈现；不作为拟稿批准。';
  }
  function render() {
    for (const key of ['state','width','theme','zoom','title','owner','origin','trust','capability']) $('pc-'+key).value=String(state[key]);
    document.documentElement.dataset.theme=state.theme;
    document.documentElement.style.setProperty('--pc-card-width',state.width+'px');
    document.documentElement.style.setProperty('--pc-inspection-zoom',state.zoom);
    setOriginalMode(); renderProposal(); updateReviewNote();
  }
  function placeDetails() {
    const dialog=$('pc-details'), zoom=Number(state.zoom), trigger=$('pc-open-details'), bounds=trigger?.getBoundingClientRect();
    const width=Math.min(360,(innerWidth-24)/zoom),height=Math.min(560,(innerHeight-24)/zoom);
    const physicalWidth=width*zoom, physicalHeight=height*zoom;
    dialog.style.width=width+'px';dialog.style.height=height+'px';
    dialog.style.left=(Math.max(12,Math.min(bounds?.right+8||12,innerWidth-physicalWidth-12))/zoom)+'px';
    dialog.style.top=(Math.max(12,Math.min(bounds?.top||12,innerHeight-physicalHeight-12))/zoom)+'px';
  }
  function openDetails() {
    focusedTrigger=$('pc-open-details');openCycle++;$('pc-copy-status').textContent='';
    renderDetailsBody();placeDetails();$('pc-details').showModal();focusedTrigger.setAttribute('aria-expanded','true');$('pc-close-details').focus({preventScroll:true});$('pc-details-body').scrollTop=0;
  }
  function closeDetails() { openCycle++;$('pc-details').close();$('pc-open-details')?.setAttribute('aria-expanded','false');(focusedTrigger?.isConnected?focusedTrigger:$('pc-open-details'))?.focus({preventScroll:true}); }
  $('pc-close-details').innerHTML=icon('x');$('pc-copy-details').innerHTML=icon('copy')+'复制全部';
  $('pc-close-details').addEventListener('click',closeDetails);
  $('pc-details').addEventListener('cancel',event=>{event.preventDefault();closeDetails();});
  $('pc-details').addEventListener('keydown',event=>{
    if(event.key!=='Tab')return;
    const controls=[...$('pc-details').querySelectorAll('button:not(:disabled),[tabindex="0"]')];
    const first=controls[0],last=controls.at(-1);
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
    else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
  });
  $('pc-copy-details').addEventListener('click',async()=>{
    if(copyPending)return;copyPending=true;const cycle=openCycle,button=$('pc-copy-details');button.setAttribute('aria-busy','true');
    const text=rawFactRows().map(([heading,rows])=>heading+'\n'+rows.map(([label,value])=>label+'：'+value).join('\n')).join('\n\n');
    try{await navigator.clipboard.writeText(text);if(cycle===openCycle)$('pc-copy-status').textContent='已复制完整详情';}
    catch{if(cycle===openCycle)$('pc-copy-status').textContent='复制失败，可在详情中选择文字';}
    finally{copyPending=false;button.setAttribute('aria-busy','false');}
  });
  for(const key of ['state','width','theme','zoom','title','owner','origin','trust','capability']) $('pc-'+key).addEventListener('change',event=>{state[key]=event.target.value;render();});
  $('pc-next-frame').addEventListener('click',()=>{if(state.state!=='live'&&state.state!=='starting'){message('设计演示：当前状态不接纳新帧；先恢复有效采集条件。');return;}state.frameIndex++;state.sampleClockMs+=5000;state.lastSuccessfulAtMs=state.sampleClockMs;state.state='live';render();message('设计演示：只有这个明确的新有效帧输入会更新画面和采集时间；未读取真实窗口。');});
  $('pc-reset').addEventListener('click',()=>{if($('pc-details').open)closeDetails();Object.assign(state,{state:'live',width:'282',theme:'dark',zoom:'1',title:'normal',owner:'candidate',origin:'candidate',trust:'selected',capability:'granted',frameIndex:0,sampleClockMs:epoch,lastSuccessfulAtMs:epoch});render();});
  addEventListener('resize',()=>{if($('pc-details').open)placeDetails();});
  window.PreviewContractDesign=Object.freeze({snapshot:()=>structuredClone({...state,displayState:displayState()}),facts:()=>structuredClone(rawFactRows()),provenance:'pending_review_authored_design_fixture_no_native_or_product_execution'});
  render();
})();
