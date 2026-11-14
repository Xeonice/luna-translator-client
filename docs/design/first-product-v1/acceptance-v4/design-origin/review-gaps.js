/* Candidate design surfaces for the six reviewed gaps. Every image/text here is
   an explicitly labelled offline replay. No system capture, Hook or model runs. */
(() => {
  'use strict';
  const esc = value => String(value ?? '').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const icon = name => window.LunaIcons?.svg(name)||'';
  const clone = value => JSON.parse(JSON.stringify(value));
  const params = new URLSearchParams(location.search);
  const view = {tab:params.get('scenario')==='session-texts'?'texts':params.get('scenario')==='session-logs'?'logs':'reading',query:'',stream:'all',level:'all',follow:true,limit:12,loadError:false};
  const sessions = new Map(), previews = new Map(); let pendingLogs = [], logSequence = 0, eventSequence = 0;
  let previewMode = 'available', lexicalMode = 'normal', pendingFollow=false;
  let activeShortcutValues={hideShortcut:'⌥ ⌘ H',pauseShortcut:'⌥ ⌘ P',modeShortcut:''},shortcutNotice='';
  view.readingLimit=12;view.newCount=0;view.newCounts={texts:0,logs:0};view.frozenEnds={texts:null,logs:null};
  function current(app) {
    const id = app.state.session.sessionId;
    if (!id) return null;
    if (!sessions.has(id)) sessions.set(id,{records:[],logs:[],continuity:[]});
    return sessions.get(id);
  }
  function startSession(app) {
    const bucket = current(app); if (!bucket) return;
    bucket.logs.push(...pendingLogs); pendingLogs=[];
    if(!bucket.logs.length){for(const [phase,message,code]of [['目标核对','运行窗口与游戏进程身份已核对','TARGET_VERIFIED'],['资源检查','取文组件已准备','HOOK_RESOURCE_READY'],['Hook 注入','已向所选目标启动接入','HOOK_ATTACH_STARTED'],['回调握手','采集端连接已建立','HOOK_HANDSHAKE_READY'],['发现文本流','收到候选来源，保留每次提取事件','STREAM_DISCOVERED']])bucket.logs.push({id:++logSequence,time:new Date().toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3}),phase,message,code,target:app.game()?.exe,level:'info',fixture:true});}
    view.limit=12; view.query=''; view.stream='all'; view.follow=true;view.newCount=0;view.newCounts={texts:0,logs:0};view.frozenEnds={texts:null,logs:null};
  }
  function recordHook(app,message) {
    const entry = {id:++logSequence,time:new Date().toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3}),level:/失败|中断|退出/.test(message)?'warning':'info',phase:/关联|身份/.test(message)?'目标核对':/选择|正文|候选/.test(message)?'文本来源':/断开|退出/.test(message)?'生命周期':'Hook 接入',message};
    const bucket = current(app);
    if (!bucket || app.state.connection?.status==='attaching') pendingLogs.push(entry); else {bucket.logs.push(entry);if(!view.follow){view.newCounts.logs++;view.newCount=view.newCounts[view.tab]||0;}}
    pendingFollow=true;
  }
  function recordExtraction(app,unit) {
    const bucket = current(app); if (!bucket || !unit) return;
    const streams = app.game()?.streams||[], selected = app.state.session.selectedStreams;
    for (const stream of streams) {
      const isBody=stream===streams[0]||stream.kind==='body'||String(stream.kind).includes('正文');
      const bodySelected=selected.includes(stream.id);
      bucket.records.push({sequence:++eventSequence,id:unit.sessionId+':raw:'+eventSequence,time:unit.time,sessionId:unit.sessionId,unitId:bodySelected?unit.id:null,streamId:stream.id,streamLabel:stream.label||stream.id,hook:stream.hook||stream.label,address:stream.address,context:stream.context,context2:stream.context2,selected:bodySelected,source:isBody?(unit.rawSource||unit.source):(stream.latestSample||stream.preview||''),scene:unit.scene,page:unit.sourceSampleId,kind:!bodySelected?'未选中来源':app.game()?.id==='never7'&&unit.source.includes('あるいは')?'同页追加':'正文',replay:true,stage:unit.beforeSelection?'选流前':'选流后'});
      bucket.logs.push({id:++logSequence,time:new Date().toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3}),level:'info',phase:'文本回调',code:'HOOK_TEXT_RECEIVED',streamId:stream.id,operationId:unit.id,target:app.game()?.exe,message:'收到文本事件 #'+eventSequence+' · '+(bodySelected?'正文来源':'未选中来源')});
    }
    pendingFollow=true;
    if(!view.follow){view.newCounts.texts+=streams.length;view.newCounts.logs+=streams.length;view.newCount=view.newCounts[view.tab]||0;}
  }
  function previewHTML(app,target,{selection=false}={}) {
    const game = target?.recordId ? window.LunaLibrary?.allGames().find(item=>item.id===target.recordId) : app.game();
    const identity = target?.hostWindowId||app.state.session.runtimeTarget?.hostWindowId||'selected-game';
    if (!previews.has(identity)) previews.set(identity,{enabled:false,paused:false,updated:null,scene:null,frameSource:null});
    const preview = previews.get(identity);
    const currentGame = game?.id===app.state.session.gameId;
    const image = currentGame ? app.state.current?.scene||target?.scene||game?.scene : target?.scene||game?.scene;
    if (preview.enabled && !preview.paused && previewMode==='available' && !(app.state.session.status==='exited'&&currentGame) && preview.frameSource!==image) {preview.scene=image;preview.frameSource=image;preview.updated=new Date().toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3});}
    const mode = app.state.session.status==='exited'&&currentGame ? 'exited' : previewMode;
    const available = preview.enabled && mode==='available' && preview.scene && !target?.self;
    const labels = {permission:'需要屏幕录制权限',exited:'目标窗口已关闭',unavailable:'暂时无法预览这个窗口'};
    return '<section class="rw-preview" '+(!selection?'id="window-preview"':'')+' data-preview-id="'+esc(identity)+'"><div class="rw-preview-heading"><div><strong>运行窗口预览</strong><small>'+esc(target?.title||game?.title||'未选择窗口')+'</small></div><span class="pill">'+(!preview.enabled?'未开启':preview.paused?'已暂停':available?'画面回放':'预览不可用')+'</span></div><div class="rw-preview-image">'+(available?'<img src="'+esc(preview.scene)+'" alt="'+esc(game?.title||target?.title)+' · 既有采集画面回放"><span class="rw-replay-badge">离线采集回放</span>':(preview.enabled&&preview.scene?'<img class="rw-stale-frame" src="'+esc(preview.scene)+'" alt="最后成功画面，当前预览不可用"><span class="rw-replay-badge">最后画面 · 离线采集回放</span>':'')+'<div class="rw-preview-empty">'+icon(mode==='permission'?'alert':'window')+'<strong>'+esc(!preview.enabled?'查看运行窗口的当前画面':labels[mode]||'此窗口没有可用预览')+'</strong><p>'+(!preview.enabled?'只预览所选窗口，画面不保存、不发送到模型。开启时按平台需要请求屏幕录制权限。':mode==='permission'?'授权后可刷新画面；当前窗口关联与取文不会因此中断。':mode==='exited'?'已停止刷新并保留会话记录。重新选择运行窗口后继续。':'保留窗口身份，可手动刷新或继续核对进程。')+'</p>'+(!preview.enabled?'<button data-rw-action="enable-preview" data-preview-target="'+esc(identity)+'">开启窗口预览</button>':mode==='permission'?'<button data-rw-action="capture-permission">查看授权说明…</button>':'')+'</div>')+'</div><div class="rw-preview-meta"><span>'+esc(target?.environment==='native_mac'?'macOS':game?.bottle||target?.bottle||'CrossOver')+' · '+esc(target?.exe||game?.exe||'—')+'<br><small>'+esc(identity)+' · '+(target?.windowsPid?'PID '+target.windowsPid:'所选运行目标')+'</small></span><div><small>画面时间 '+esc(preview.updated||'尚无成功画面')+'</small><div class="rw-preview-actions"><button data-rw-action="refresh-preview" data-preview-target="'+esc(identity)+'" '+(!preview.enabled||mode==='exited'?'disabled':'')+'>'+icon('history')+' 刷新</button><button data-rw-action="pause-preview" data-preview-target="'+esc(identity)+'" '+(!preview.enabled?'disabled':'')+' aria-pressed="'+preview.paused+'">'+icon(preview.paused?'play':'pause')+' '+(preview.paused?'继续预览':'暂停预览')+'</button>'+(preview.enabled?'<button data-rw-action="close-preview" data-preview-target="'+esc(identity)+'">关闭</button>':'')+'</div></div></div></section>';
  }
  function tabsHTML() {
    return '<div class="rw-session-tabs" role="tablist" aria-label="当前会话内容">'+[['reading','正文历史'],['texts','全部提取'],['logs','Hook 日志']].map(([id,label])=>'<button type="button" role="tab" id="session-tab-'+id+'" aria-controls="session-panel-'+id+'" aria-selected="'+(view.tab===id)+'" tabindex="'+(view.tab===id?'0':'-1')+'" data-rw-tab="'+id+'">'+label+'</button>').join('')+'</div>';
  }
  function pauseFollowing(app) {
    if(!view.follow)return;
    const bucket=current(app);view.follow=false;view.newCount=0;view.newCounts={texts:0,logs:0};
    view.frozenEnds={texts:bucket?.records.at(-1)?.sequence??0,logs:bucket?.logs.at(-1)?.id??0};
  }
  function sessionHTML(app,readingHTML) {
    if (!app.state.session.gameId) return readingHTML;
    const header = readingHTML.match(/^<header[\s\S]*?<\/header>/)?.[0]||'';
    const reading = readingHTML.slice(header.length)+readingHistoryHTML(app);
    const game = app.game(), bucket=current(app), total=view.tab==='logs'?bucket.logs.length:bucket.records.length;
    const records = (view.tab==='logs'?bucket.logs:bucket.records).filter(record=>(!view.query||(record.message||record.source||'').toLowerCase().includes(view.query.toLowerCase()))&&(view.tab==='logs'?(view.level==='all'||record.level===view.level):(view.stream==='all'||record.streamId===view.stream)));
    const end=view.frozenEnds[view.tab];
    const windowRecords=!view.follow&&end!==null?records.filter(record=>(view.tab==='logs'?record.id:record.sequence)<=end):records;
    const visible = windowRecords.slice(Math.max(0,windowRecords.length-view.limit)), activeId=view.tab==='logs'?'hook-events':'session-history';
    const runtime=app.state.session.runtimeTarget;
    const body=view.tab==='reading'?'<div class="rw-reading-grid"><div>'+reading+'</div><div class="rw-preview-column">'+previewHTML(app,runtime)+'</div></div>':'<section id="'+activeId+'" class="rw-session-list"><div class="rw-list-intro"><div><h2>'+(view.tab==='logs'?'Hook 接入日志':'本次会话的全部提取文本')+'</h2><p>'+(view.tab==='logs'?'从目标核对、接入到恢复的事件，按发生顺序保留。':'所有已接收来源均可查看；未选中的流不进入阅读字幕。')+'</p></div><span class="pill">共 '+total+' 条</span></div><div class="rw-list-toolbar"><label class="rw-search">'+icon('search')+'<input id="rw-session-search" aria-label="'+(view.tab==='logs'?'搜索 Hook 日志':'搜索提取文本')+'" placeholder="搜索'+(view.tab==='logs'?'日志':'原文')+'" value="'+esc(view.query)+'"></label><select id="rw-session-filter" aria-label="'+(view.tab==='logs'?'日志级别':'文本来源')+'">'+(view.tab==='logs'?'<option value="all">所有级别</option><option value="info" '+(view.level==='info'?'selected':'')+'>常规事件</option><option value="warning" '+(view.level==='warning'?'selected':'')+'>警告与失败</option>':'<option value="all">全部文本来源</option>'+(game.streams||[]).map(stream=>'<option value="'+esc(stream.id)+'" '+(view.stream===stream.id?'selected':'')+'>'+esc(stream.label)+' · '+(app.state.session.selectedStreams.includes(stream.id)?'已选正文':'未选中')+'</option>').join(''))+'</select><button data-rw-action="toggle-follow" aria-pressed="'+view.follow+'">'+icon(view.follow?'check':'pause')+' '+(view.follow?'跟随新内容':'回到最新'+(view.newCount?' · '+view.newCount+' 条新内容':''))+'</button><button data-rw-action="export-session">导出…</button></div>'+(app.state.session.integrityLost?'<div class="status-banner warning"><div><strong>接收曾中断，完整性待确认</strong><p>中断前的记录全部保留。恢复从新句继续，不补造缺失文本。</p></div><button data-action="recover">恢复接收…</button></div>':'')+'<div class="rw-records" role="log" aria-label="'+(view.tab==='logs'?'Hook 事件列表':'全量提取文本列表')+'" aria-live="off">'+(visible.length?visible.map(record=>view.tab==='logs'?'<article class="rw-log-entry" data-hook-log="'+record.id+'"><span class="rw-sequence">#'+record.id+'</span><div><div class="rw-record-meta"><strong>'+esc(record.phase)+'</strong><span class="pill '+(record.level==='warning'?'warn':'')+'">'+(record.level==='warning'?'警告':'信息')+'</span><time>'+esc(record.time)+'</time></div><p>'+esc(record.message)+'</p><details><summary>查看事件详情</summary><dl><div><dt>事件代码</dt><dd>'+esc(record.code||'HOOK_LIFECYCLE_EVENT')+'</dd></div><div><dt>目标</dt><dd>'+esc(record.target||game.exe)+'</dd></div><div><dt>流 / 操作</dt><dd>'+esc(record.streamId||'—')+' / '+esc(record.operationId||app.state.session.sessionId)+'</dd></div></dl></details></div></article>':'<article class="rw-text-entry" data-extraction-sequence="'+record.sequence+'"><span class="rw-sequence">#'+record.sequence+'</span><div><div class="rw-record-meta"><strong>'+esc(record.streamLabel)+'</strong><span class="pill">'+esc(record.kind)+'</span>'+(!record.selected?'<span class="muted">未进入字幕</span>':'')+'<time>'+esc(record.time)+'</time></div><p lang="'+(game.language==='zh-CN'?'zh-CN':'ja')+'">'+esc(record.source)+'</p><details><summary>查看完整来源</summary><dl><div><dt>会话</dt><dd>'+esc(record.sessionId)+'</dd></div><div><dt>文本流</dt><dd>'+esc(record.streamId)+'</dd></div><div><dt>Hook / 地址</dt><dd>'+esc(record.hook)+' / '+esc(record.address||'—')+'</dd></div><div><dt>上下文</dt><dd>'+esc(record.context??'—')+' / '+esc(record.context2??'—')+'</dd></div><div><dt>阅读单元</dt><dd>'+esc(record.unitId||'未形成正文')+'</dd></div><div><dt>原文状态</dt><dd>'+esc(record.kind)+' · '+esc(record.stage)+' · 逐次保留</dd></div></dl></details></div></article>').join(''):'<div class="rw-list-empty"><strong>'+(view.query?'没有匹配结果':'等待收到'+(view.tab==='logs'?'接入事件':'原文'))+'</strong><p>'+(view.query?'试试其他关键词或清除筛选。':'收到后会按顺序出现在这里。')+'</p></div>')+'</div><div class="rw-list-footer"><span>已显示 '+visible.length+' / '+records.length+' 条'+(view.query?'匹配记录':'')+'</span>'+(view.loadError?'<span class="rw-inline-error" role="alert">加载较早记录失败，已显示内容保留。</span><button data-rw-action="retry-more">重试</button>':visible.length<windowRecords.length?'<button data-rw-action="load-more">加载更多记录</button>':'<span class="muted">已到记录末尾</span>')+'</div></section>';
    return header+tabsHTML()+'<div id="session-panel-'+view.tab+'" class="rw-session-panel" role="tabpanel" aria-labelledby="session-tab-'+view.tab+'">'+body+'</div>';
  }
  function readingTranslation(app,record) {
    const unit=app.state.history.find(item=>item.id===record.unitId);
    if(!unit)return '<small class="rw-history-translation">尚未形成阅读结果</small>';
    return '<p class="rw-history-translation">'+esc(unit.editedTranslation||unit.translation||({pending:'正在翻译…',streaming:'正在接收译文…',failed:'翻译失败，原文保留',disabled:'自动翻译未启用',cancelled:'翻译已取消，原文保留'}[unit.translationStatus]||'原文已为目标语言'))+'</p>';
  }
  function readingHistoryHTML(app) {
    const records=(current(app)?.records||[]).filter(record=>record.selected), shown=[...records].reverse().slice(0,view.readingLimit);
    return '<section class="rw-reading-history" id="session-body-history"><div class="section-heading"><h3>本次会话正文历史</h3><span class="pill">'+records.length+' 条</span></div><p class="help-note">按接收顺序保留每次正文；完整原始内容及其他来源在“提取文本”中查看。</p><div class="rw-records">'+shown.map(record=>'<article class="rw-text-entry"><span class="rw-sequence">#'+record.sequence+'</span><div><div class="rw-record-meta"><strong>'+esc(record.streamLabel)+'</strong><time>'+esc(record.time)+'</time></div><p>'+esc(record.source)+'</p>'+readingTranslation(app,record)+'</div></article>').join('')+'</div><div class="rw-list-footer"><span>已显示 '+shown.length+' / '+records.length+' 条正文</span>'+(shown.length<records.length?'<button data-rw-action="load-reading-history">加载较早正文</button>':'<span class="muted">已到记录起点</span>')+'</div></section>';
  }
  function shortcut(value) {
    const raw=String(value||'').trim();if(!raw)return {empty:true};
    const text=raw.toLowerCase().replace(/command|cmd|meta|⌘/g,' meta ').replace(/option|alt|⌥/g,' alt ').replace(/control|ctrl|⌃/g,' ctrl ').replace(/shift|⇧/g,' shift ').replace(/\+/g,' ').trim();
    const parts=text.split(/\s+/),key=parts.filter(part=>!['meta','alt','ctrl','shift'].includes(part)).join('');
    if(!/^[a-z]$/.test(key)||!parts.some(part=>['meta','alt','ctrl'].includes(part)))return {error:'请包含 Command、Option 或 Control，并搭配一个英文字母。'};
    const config={key,meta:parts.includes('meta'),alt:parts.includes('alt'),ctrl:parts.includes('ctrl'),shift:parts.includes('shift')};
    if(config.meta&&!config.alt&&!config.ctrl&&['q','c','v','x','a','z','s','w'].includes(key))return {error:'与常用系统或编辑快捷键冲突，请换一个组合。'};
    return {...config,signature:[config.meta,config.alt,config.ctrl,config.shift,key].join(':')};
  }
  function shortcutStatus(app) {
    const fields=[['hideShortcut','显示 / 隐藏浮窗'],['pauseShortcut','暂停 / 继续取文'],['modeShortcut','原文与译文模式']];
    const parsed=fields.map(([field,label])=>({field,label,...shortcut(app.state.settings.display[field])}));
    const errors=parsed.filter(item=>item.error).map(item=>item.label+'：'+item.error);
    for(let i=0;i<parsed.length;i++)for(let j=i+1;j<parsed.length;j++)if(parsed[i].signature&&parsed[i].signature===parsed[j].signature)errors.push(parsed[i].label+'与'+parsed[j].label+'使用了同一组合，请修改。');
    return {parsed,errors};
  }
  function shortcutAction(event,app) {
    if(event.repeat||event.isComposing||event.target.closest('input,textarea,[contenteditable=true]'))return null;
    const status=shortcutStatus({state:{settings:{display:activeShortcutValues}}});if(status.errors.length)return null;
    const key=event.code?.startsWith('Key')?event.code.slice(3).toLowerCase():event.key.toLowerCase();
    const match=status.parsed.find(item=>item.signature&&item.key===key&&item.meta===event.metaKey&&item.alt===event.altKey&&item.ctrl===event.ctrlKey&&item.shift===event.shiftKey);
    return match?({hideShortcut:'toggle-hud',pauseShortcut:'toggle-pause',modeShortcut:'cycle-display-mode'}[match.field]):null;
  }
  function renderHUD(app) {
    const s=app.state,unit=s.current,opts=window.LunaSettings.readingOptions(s.settings),hud=document.getElementById('hud-window');
    for(const [id,enabled]of [['segmentation',opts.segmentation],['furigana',opts.furigana],['pos',opts.pos]])document.getElementById('hud-'+id)?.setAttribute('aria-pressed',String(enabled));
    const pos=document.getElementById('hud-pos');pos.disabled=!opts.segmentation;pos.title=opts.segmentation?'词性提示':'启用分词后可显示词性提示';
    const translationsEnabled=s.session.translationEnabled;
    const translateButton=document.getElementById('hud-translation-toggle');translateButton.setAttribute('aria-pressed',String(translationsEnabled));translateButton.setAttribute('aria-label',translationsEnabled?'关闭自动翻译':'启用自动翻译');translateButton.title=translationsEnabled?'关闭自动翻译':'启用自动翻译';
    const pauseButton=document.getElementById('hud-pause');pauseButton.disabled=s.session.status!=='connected';pauseButton.title=pauseButton.getAttribute('aria-label')+' '+(activeShortcutValues.pauseShortcut||'');
    for(const id of ['hud-retranslate','hud-copy','hud-learn'])document.getElementById(id).disabled=!unit;
    const pin=document.getElementById('hud-pin');pin.setAttribute('aria-pressed',String(s.settings.display.pin!==false));pin.title=s.settings.display.pin!==false?'取消置顶':'始终置顶';pin.setAttribute('aria-label',pin.title);
    const size=document.getElementById('hud-size');size.title=opts.preset==='compact'?'切换舒适布局':'切换紧凑布局';size.setAttribute('aria-label',size.title);
    const hide=document.getElementById('hud-hide');hide.title='隐藏浮窗 '+(activeShortcutValues.hideShortcut||'');
    const mode=s.displayMode||'both',source=document.getElementById('hud-source'),translation=document.getElementById('hud-translations');
    source.hidden=mode==='translation'&&!!unit?.translation&&unit.translationStatus==='complete';
    translation.querySelectorAll('.hud-translation').forEach(node=>node.hidden=mode==='source');
    const modeButton=document.getElementById('hud-display-mode');modeButton.textContent={both:'原文＋译文',source:'仅原文',translation:'仅译文'}[mode];modeButton.setAttribute('aria-label','字幕内容模式：'+modeButton.textContent);modeButton.title='切换原文与译文显示 '+(activeShortcutValues.modeShortcut||'');
    hud.querySelector('.hud-language-note')?.remove();
    const japanese=unit?.language!=='zh-CN';
    let note='';
    if(japanese&&lexicalMode==='error')note='词法暂不可用，原文已保留。注音与词性待恢复后显示。';
    else if(japanese&&lexicalMode==='unknown')note='部分词语读音未知，保留原字，不推测注音。';
    if(mode==='translation'&&!source.hidden)note+=(note?' ':'')+'当前尚无完整译文，继续保留原文。';
    if(note)document.getElementById('hud-reading').insertAdjacentHTML('beforeend','<div class="hud-language-note" role="status">'+esc(note)+(lexicalMode==='error'?'<button data-rw-action="retry-language">重试分词</button>':'')+'</div>');
  }
  function afterRender(app) {
    const followButton=document.querySelector('[data-rw-action="toggle-follow"]');if(followButton&&!view.follow)followButton.textContent='回到最新'+(view.newCount?' · '+view.newCount+' 条新内容':'');
    const statusNode=document.getElementById('rw-shortcut-status');if(statusNode){const status=shortcutStatus(app);statusNode.textContent=(status.errors.join(' ')||(shortcutNotice||'修改后需应用快捷键。'))+' 当前生效：显隐 '+(activeShortcutValues.hideShortcut||'未设置')+'；暂停 '+(activeShortcutValues.pauseShortcut||'未设置')+'；原译 '+(activeShortcutValues.modeShortcut||'未设置')+'。仅在设计页面内响应，系统全局注册待实施。';statusNode.classList.toggle('ls-error-text',status.errors.length>0);}
    if(pendingFollow&&view.follow&&app.state.page==='session'&&['texts','logs'].includes(view.tab)&&!view.query){const content=document.getElementById('main-content');content.scrollTop=content.scrollHeight;}
    pendingFollow=false;
  }
  function componentHTML() {
    return '<section class="rw-component-page" id="select-showcase"><header><span class="pill">组件与交互 · 设计候选</span><h1>同一套轻量控件</h1><p>中性的暗蓝灰材质、统一线性图标和清楚的交互反馈。</p></header><div class="rw-component-grid"><article><h2>专用选择器</h2><label>当前文本来源<select id="showcase-normal" aria-label="组件示例文本来源"><option value="body">Artemis · 正文</option><option value="unselected">Artemis · 界面文本</option><option value="long">GameN7_Steam.exe · Steam · MAGES2 · 长文本来源名称完整显示</option><option value="disabled" disabled>已退出来源 · 不可选择</option></select></label><label>空列表<select id="showcase-no-options" aria-label="空模型列表" data-placeholder="暂无可选模型"></select></label><label>未选择<select id="showcase-empty" aria-label="未选择模型"><option value="">选择模型…</option><option value="study">日语阅读模型</option></select></label><label>正在获取<select id="showcase-loading" aria-label="正在获取模型" disabled data-select-state="loading"><option>正在获取模型…</option></select></label><label>获取失败<select id="showcase-error" aria-label="获取失败的模型" disabled data-select-state="error"><option>无法获取模型列表</option></select></label><div class="rw-control-state"><span id="showcase-error-message">已保存的配置仍保留。</span><button data-rw-action="retry-showcase">重试</button></div><label>不可用<select id="showcase-disabled" aria-label="不可用模型" disabled><option>连接已停用</option></select></label><small>方向键移动 · Enter 确认 · Esc 取消；长文本可在展开面板完整查看。</small></article><article><h2>按钮与图标</h2><div class="rw-control-state"><span>默认</span><button>'+icon('window')+' 选择窗口…</button></div><div class="rw-control-state"><span>主要操作</span><button class="primary">关联并接入</button></div><div class="rw-control-state"><span>不可用</span><button disabled>'+icon('play')+' 开始阅读</button></div><div class="rw-control-state"><span>已选中</span><button aria-pressed="true">'+icon('check')+' 显示注音</button></div><p>悬停与键盘聚焦使用同一材质层级；图标不另加彩色底块。</p></article><article id="ruby-material"><h2>字幕与语言呈现</h2><p>在游戏上的浮窗中直接开启分词、注音和词性，保留原文的字序与标点。注音可独立于可点击分词显示。</p><div class="rw-component-actions"><button data-action="show-hud">查看浮窗</button><button data-rw-action="open-reading-settings">阅读外观与快捷键…</button></div></article><article id="hud-controls"><h2>移动与恢复</h2><p>拖动浮窗顶部手柄或底部空白处。正文可选词和滚动，操作按钮可直接点击。隐藏再打开保留位置。</p><button data-rw-action="reset-hud">'+icon('history')+' 恢复浮窗位置</button><p>⌥⌘H 显示 / 隐藏 · ⌥⌘P 暂停 / 继续。快捷键可在“设置 → 显示”修改。</p><small>网页中响应演示；系统全局注册、重启持久化和跨显示器恢复须正式包实测。</small></article></div></section>';
  }
  function stopPreview(target) {
    if (!target?.hostWindowId) return;
    const preview=previews.get(target.hostWindowId);if(preview)Object.assign(preview,{enabled:false,paused:false,scene:null,frameSource:null,updated:null});
  }
  function renderPreviewNodes(app) {
    document.querySelectorAll('[data-preview-id]').forEach(node=>{
      const choice=window.LunaLibrary?.windowChoices().find(item=>item.hostWindowId===node.dataset.previewId)||app.state.session.runtimeTarget;
      node.outerHTML=previewHTML(app,choice,{selection:!!node.closest('dialog')});
    });
    window.LunaControls?.upgrade();
  }
  function setup(app) {
    const bar=document.querySelector('.review-bar>div');
    bar.insertAdjacentHTML('beforeend','<label>预览状态 <select id="review-preview"><option value="available">画面可用</option><option value="permission">未授权</option><option value="unavailable">预览不可用</option><option value="exited">窗口退出</option></select></label><label>语言状态 <select id="review-lexical"><option value="normal">正常标注</option><option value="unknown">部分读音未知</option><option value="error">词法不可用</option></select></label><button id="review-components">组件状态</button>');
    document.getElementById('review-components').addEventListener('click',()=>{app.state.page='components';app.openWindow('main');});
    document.getElementById('review-preview').addEventListener('change',event=>{previewMode=event.target.value;app.render();renderPreviewNodes(app);});
    document.getElementById('review-lexical').addEventListener('change',event=>{lexicalMode=event.target.value;app.render();});
    if (['session-texts','session-logs','window-preview','components'].includes(params.get('scenario'))) {app.state.windows.main=true;app.state.page=params.get('scenario')==='components'?'components':'session';}
    if(params.get('scenario')==='hud-plain'){app.state.settings.general.segmentation=false;app.state.settings.general.furigana=false;app.state.settings.general.pos=false;}
    if(params.get('scenario')==='hud-ruby'){app.state.settings.general.segmentation=false;app.state.settings.general.furigana=true;}
    if(params.get('scenario')==='hud-reduced')app.state.settings.preferences.appearance='reduced';
    if(params.get('scenario')==='hud-unknown')lexicalMode='unknown';
    if(params.get('scenario')==='hud-lexical-error')lexicalMode='error';
    const failure=bar.querySelector('#review-fault');failure.insertAdjacentHTML('beforeend','<option value="history-load">会话较早记录加载失败</option>');failure.addEventListener('change',event=>{view.loadError=event.target.value==='history-load';});
  }
  function exportSession(app) {
    const bucket=current(app);if(!bucket)return;
    app.dialog({title:'导出当前会话',description:'文件会包含游戏原文及可能识别的程序路径，请确认保存位置和分享范围。',body:'<label class="rw-export-field">内容范围<select id="rw-export-kind" aria-label="导出内容范围"><option value="body">正文历史</option><option value="texts" selected>全部提取</option><option value="logs">Hook 日志</option></select></label><label class="rw-export-field">记录范围<select id="rw-export-scope" aria-label="导出记录范围"><option value="session">完整会话</option><option value="filtered">当前过滤结果</option></select></label><p class="muted">此处预览本离线设计数据，不写入系统文件。正式客户端由你明确选择保存位置。</p>',confirmLabel:'预览导出内容',onConfirm:()=>{
      const kind=document.getElementById('rw-export-kind').value,scope=document.getElementById('rw-export-scope').value;
      let records=kind==='logs'?bucket.logs:bucket.records.filter(record=>kind!=='body'||record.selected);
      if(scope==='filtered')records=records.filter(record=>(!view.query||(record.message||record.source||'').includes(view.query))&&(kind==='logs'?(view.level==='all'||record.level===view.level):(view.stream==='all'||record.streamId===view.stream)));
      app.closeDialog();app.dialog({title:'导出内容预览 · '+records.length+' 条',description:(scope==='session'?'完整会话':'当前过滤结果')+' · 离线设计样本，不包含预览帧或凭据。',body:'<textarea readonly rows="12" aria-label="导出全文">'+esc(JSON.stringify({sessionId:app.state.session.sessionId,scope,kind,records},null,2))+'</textarea>',confirmLabel:'完成',cancelLabel:'关闭'});return false;
    }});
  }
  document.addEventListener('wheel',event=>{if(event.deltaY>=0||!event.target.closest('#main-content')||window.LunaDesign?.state.page!=='session'||!view.follow||view.tab==='reading')return;pauseFollowing(window.LunaDesign);const button=document.querySelector('[data-rw-action="toggle-follow"]');if(button){button.setAttribute('aria-pressed','false');button.textContent='回到最新';}},{passive:true});
  function resetHUD(app) {const hud=document.getElementById('hud-window');hud.style.left='calc(50% - 480px)';hud.style.top='450px';hud.style.right='auto';delete app.state.draggedWindows.hud;app.notify('浮窗已恢复到游戏下方；阅读内容保留。');}
  document.addEventListener('click',event=>{
    const app=window.LunaDesign;if(!app)return;
    const tab=event.target.closest('[data-rw-tab]');if(tab){view.tab=tab.dataset.rwTab;view.newCount=view.newCounts[view.tab]||0;view.query='';view.limit=12;app.render();document.getElementById('session-tab-'+view.tab)?.focus({preventScroll:true});return;}
    const button=event.target.closest('[data-rw-action]');if(!button)return;
    const a=button.dataset.rwAction;
    if(a==='toggle-follow'){if(view.follow)pauseFollowing(app);else{view.follow=true;view.newCount=0;view.newCounts={texts:0,logs:0};view.frozenEnds={texts:null,logs:null};pendingFollow=true;}app.render();}
    if(a==='load-more'||a==='retry-more'){view.loadError=false;view.limit+=12;app.render();}
    if(a==='load-reading-history'){view.readingLimit+=12;app.render();}
    if(a==='retry-showcase'){const select=document.getElementById('showcase-error');select.innerHTML='<option value="study">日语阅读模型</option><option value="translation">实时翻译模型</option>';select.disabled=false;delete select.dataset.selectState;const message=document.getElementById('showcase-error-message');if(message)message.textContent='已重新获取设计示例，当前仍未应用。';window.LunaControls?.sync(select);}
    if(a==='enable-preview'){const preview=previews.get(button.dataset.previewTarget);if(preview){preview.enabled=true;preview.paused=false;renderPreviewNodes(app);}return;}
    if(a==='close-preview'){const preview=previews.get(button.dataset.previewTarget);if(preview){preview.enabled=false;preview.scene=null;preview.frameSource=null;preview.updated=null;renderPreviewNodes(app);}return;}
    if(a==='refresh-preview'||a==='pause-preview'){const preview=previews.get(button.dataset.previewTarget);if(preview){if(a==='pause-preview')preview.paused=!preview.paused;else{preview.paused=false;}renderPreviewNodes(app);}}
    if(a==='capture-permission')app.dialog({title:'允许显示游戏窗口画面',description:'正式客户端需要 macOS 屏幕录制权限，授权只用于所选窗口的预览。取文接入独立运行。',body:'<p>系统设置 → 隐私与安全性 → 屏幕录制，允许 Luna 后返回并刷新预览。</p><p class="muted">本离线设计稿不会请求权限或读取屏幕。</p>',confirmLabel:'知道了',cancelLabel:'关闭'});
    if(a==='retry-language'){lexicalMode='normal';document.getElementById('review-lexical').value='normal';app.render();}
    if(a==='apply-shortcuts'){const status=shortcutStatus(app);if(status.errors.length){shortcutNotice='应用失败，保留原生效组合。';app.render();}else{activeShortcutValues=Object.fromEntries(['hideShortcut','pauseShortcut','modeShortcut'].map(key=>[key,app.state.settings.display[key]||'']));shortcutNotice='快捷键已在本设计页面内应用。';app.render();}}
    if(a==='clear-shortcuts'){for(const key of ['hideShortcut','pauseShortcut','modeShortcut'])app.state.settings.display[key]='';shortcutNotice='已清空草稿，应用后生效。';app.render();}
    if(a==='restore-shortcuts'){Object.assign(app.state.settings.display,{hideShortcut:'⌥ ⌘ H',pauseShortcut:'⌥ ⌘ P',modeShortcut:''});shortcutNotice='已恢复默认候选，应用后生效。';app.render();}
    if(a==='export-session')exportSession(app);
    if(a==='reset-hud')resetHUD(app);
    if(a==='open-reading-settings'){app.state.settings.category='display';app.openWindow('settings');}
  });
  document.addEventListener('input',event=>{if(event.target.id!=='rw-session-search')return;const app=window.LunaDesign;pauseFollowing(app);view.query=event.target.value;view.limit=12;const caret=event.target.selectionStart;app.render();const input=document.getElementById('rw-session-search');input?.focus({preventScroll:true});input?.setSelectionRange(caret,caret);});
  document.addEventListener('input',event=>{if(event.target.matches('[data-settings-field^="display."]'))afterRender(window.LunaDesign);});
  document.addEventListener('change',event=>{if(event.target.id!=='rw-session-filter')return;pauseFollowing(window.LunaDesign);if(view.tab==='logs')view.level=event.target.value;else view.stream=event.target.value;view.limit=12;window.LunaDesign.render();});
  document.addEventListener('keydown',event=>{const tab=event.target.closest('[data-rw-tab]');if(!tab||!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();const tabs=[...document.querySelectorAll('[data-rw-tab]')],index=tabs.indexOf(tab),next=event.key==='Home'?0:event.key==='End'?tabs.length-1:(index+(event.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;tabs[next].click();});
  window.LunaReviewGaps={setup,activeShortcuts:()=>({...activeShortcutValues}),renderHUD,shortcutAction,shortcutStatus,afterRender,sessionHTML,componentHTML,previewHTML,stopPreview,startSession,recordHook,recordExtraction,resetHUD,pauseFollowing,previews,lexicalMode:()=>lexicalMode,view,sessions};
})();
