/* Offline high-fidelity design. No service requests, real keys or production persistence. */
(function () {
  'use strict';
  const names = {translation:'实时翻译',grammar:'句子精读',word:'语境查词'};
  const categories = [
    ['general','settings','通用'],['input','plug','输入与HOOK'],['ocr','scan','OCR引擎'],['translation','languages','翻译服务'],
    ['processing','sliders','文本处理'],['speech','volume','语音'],['display','type','显示与快捷键'],
    ['network','globe','网络服务'],['data','database','配置与数据'],['diagnostics','activity','更新与诊断']
  ];
  const icon = name => window.LunaIcons ? window.LunaIcons.svg(name) : '';
  const clone = value => JSON.parse(JSON.stringify(value));
  const esc = value => String(value == null ? '' : value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const protocolNames = {traditional:'Microsoft 文本翻译',openai:'OpenAI 兼容',anthropic:'Anthropic Messages'};
  const defaultTemplate = {translation:'将以下日文译为自然中文。只返回译文。\n{{source}}\n{{history}}\n{{terms}}',grammar:'解释以下原句的句子结构与语法。\n{{source}}\n{{history}}\n{{dictionary}}',word:'解释选词在原句中的意思。\n词：{{word}}\n原句：{{source}}\n{{dictionary}}'};
  function resources() {
    return {
      connections:{
        microsoft:{id:'microsoft',name:'Microsoft 翻译',protocol:'traditional',location:'cloud',endpoint:'https://api.cognitive.microsofttranslator.com',auth:'key',credential:'keyref:demo-microsoft-v1',proxy:'inherit',enabled:true,version:1},
        cloud:{id:'cloud',name:'云端学习服务',protocol:'openai',location:'cloud',endpoint:'https://api.example.invalid/v1',auth:'key',credential:'keyref:demo-study-v1',proxy:'inherit',enabled:true,version:1},
        local:{id:'local',name:'本机模型服务',protocol:'openai',location:'local',endpoint:'http://127.0.0.1:8080/v1',auth:'none',credential:'',proxy:'direct',enabled:true,version:1}
      },
      models:{
        traditional:{id:'traditional',name:'Microsoft · 日中翻译',connectionId:'microsoft',modelId:'',capability:'translation',outputMode:'text',template:defaultTemplate.translation,templatePurpose:'translation',temperatureMode:'omit',temperature:'',maxTokens:'',version:1},
        study:{id:'study',name:'精读模型',connectionId:'cloud',modelId:'study-model-example',capability:'structured',outputMode:'structured',template:defaultTemplate.grammar,templatePurpose:'grammar',temperatureMode:'set',temperature:'0.2',maxTokens:'2048',version:1},
        quick:{id:'quick',name:'轻量查词模型',connectionId:'cloud',modelId:'word-model-example',capability:'text',outputMode:'text',template:defaultTemplate.word,templatePurpose:'word',temperatureMode:'set',temperature:'0.1',maxTokens:'1024',version:1},
        local:{id:'local',name:'本机日文模型',connectionId:'local',modelId:'local-japanese-example',capability:'text',outputMode:'text',template:defaultTemplate.translation,templatePurpose:'translation',temperatureMode:'inherit',temperature:'0.4',maxTokens:'1024',version:1}
      },
      bindings:{translation:{modelId:'traditional',enabled:true,historyCount:0,context:'original',outputMode:'text',language:'简体中文',timeout:60,retries:0,concurrency:1,temperatureMode:'inherit',temperature:'',templateMode:'inherit',template:'',fallback:'off',cache:true,auto:true,extraModels:[]},grammar:{modelId:'study',enabled:false,historyCount:0,context:'original',outputMode:'structured',language:'简体中文',timeout:60,retries:0,concurrency:1,temperatureMode:'inherit',temperature:'',templateMode:'inherit',template:'',fallback:'off',cache:true,auto:false,extraModels:[]},word:{modelId:'quick',enabled:false,historyCount:0,context:'original',outputMode:'text',language:'简体中文',timeout:60,retries:0,concurrency:1,temperatureMode:'inherit',temperature:'',templateMode:'inherit',template:'',fallback:'off',cache:true,auto:false,extraModels:[]}},
      localEngines:window.LunaLocalTranslation.resources(),
      overrides:{}
    };
  }
  function initialState() {
    const draft=resources(), applied={};
    for(const purpose of Object.keys(names)) applied['global:'+purpose]={snapshot:clone(draft),revision:1,enabled:draft.bindings[purpose].enabled,source:'全局用途',savedAt:'10:24'};
    return {engineFamily:'llm',category:'translation',tab:'purposes',purpose:'translation',scope:'global',selectedConnection:'cloud',selectedModel:'study',draft,saved:clone(draft),applied,revision:1,dirty:false,notice:'',noticeKind:'info',tests:{},testTarget:'model',testOutcome:'pass',testState:'idle',modelListState:'idle',blockedConnections:{},general:{furigana:true,segmentation:true,pos:true,dictionary:true,automatic:true,launch:false,history:true},preferences:{preset:'comfortable',fontScale:1,appearance:'standard'},display:{fontSize:22,sourceSize:18,opacity:82,pin:true,hideShortcut:'⌥ ⌘ H',pauseShortcut:'⌥ ⌘ P',modeShortcut:''},input:{attach:true,dedupe:true,merge:120},keyPool:{enabled:false,strategy:'roundrobin',keys:[{reference:'keyref:demo-study-v1',status:'未测试'},{reference:'keyref:demo-study-v2',status:'未测试'}]},importState:'idle',diagnostic:'尚未生成诊断包'};
  }
  function resetBindings(s) {
    Object.keys(names).forEach(p=>{s.draft.bindings[p].enabled=false;s.applied['global:'+p].enabled=false;s.applied['global:'+p].snapshot.bindings[p].enabled=false;});
    s.saved=clone(s.draft);s.notice='首次使用：内置词典已就绪，在线用途尚未启用。';
  }
  function currentGameId(app) { return app.state.selectedGame || app.state.session && app.state.session.gameId || null; }
  function gameName(app,id) { const d=window.LunaDesignData;const games=window.LunaLibrary?window.LunaLibrary.allGames():d&&d.games||[];const g=games.find(x=>x.id===id);return g?g.title:'当前游戏'; }
  function override(d,gameId,purpose,create) {
    if(!d.overrides[gameId]&&create)d.overrides[gameId]={};
    if(d.overrides[gameId]&&!d.overrides[gameId][purpose]&&create)d.overrides[gameId][purpose]={mode:'inherit',backup:clone(d.bindings[purpose])};
    return d.overrides[gameId]&&d.overrides[gameId][purpose];
  }
  function resolved(d,purpose,gameId) {
    const o=gameId&&override(d,gameId,purpose,false);
    if(o&&o.mode==='off')return {binding:Object.assign({},d.bindings[purpose],{enabled:false}),source:'游戏专用 · 已关闭'};
    if(o&&o.mode==='override')return {binding:o.backup,source:'游戏专用覆盖'};
    return {binding:d.bindings[purpose],source:gameId?'继承全局用途':'全局用途'};
  }
  function effective(s,purpose,gameId) {
    const local=gameId&&s.applied[gameId+':'+purpose];
    const entry=local||s.applied['global:'+purpose];
    if(!entry)return {enabled:false,connectionEnabled:false,modelId:'',modelName:'未配置',connectionName:'未配置',outputMode:'text',historyCount:0,status:'待配置',revision:0,source:'无绑定'};
    let result=resolved(entry.snapshot,purpose,local?gameId:null);
    if(local&&result.source==='继承全局用途') {
      const g=s.applied['global:'+purpose];if(g){result=resolved(g.snapshot,purpose,null);result.source='继承全局用途';}
    }
    const b=result.binding||{}, sourceSnapshot=local&&result.source==='继承全局用途'&&s.applied['global:'+purpose]?s.applied['global:'+purpose].snapshot:entry.snapshot;
    if(window.LunaLocalTranslation.isLocal(b.modelId))return window.LunaLocalTranslation.effective(s,activeLocalEntry(s,entry,local,result,purpose),b,result.source,b.modelId,purpose,(local&&result.source!=='继承全局用途'?gameId:'global')+':'+purpose);
    const m=sourceSnapshot.models[b.modelId],c=m&&sourceSnapshot.connections[m.connectionId],connectionEnabled=!!c&&!s.blockedConnections[c.id]&&c.enabled!==false;
    const enabled=!!b.enabled&&!!m&&!!c&&connectionEnabled;
    const activeEntry=local&&result.source==='继承全局用途'?s.applied['global:'+purpose]:entry;
    const matching=Object.values(s.tests).find(t=>t.target===purpose&&t.fingerprint===testFingerprint(sourceSnapshot,purpose,local&&result.source!=='继承全局用途'?gameId:null)&&t.status==='pass');
    return {enabled,connectionEnabled,modelId:m?m.modelId:'',modelName:m?m.name:'未选择模型',connectionName:c?c.name:'未选择连接',outputMode:b.outputMode||m&&m.outputMode||'text',historyCount:Number(b.historyCount)||0,status:!b.enabled?'未启用':!connectionEnabled?'连接已停用':matching?'用途样例通过':'已启用 · 未验证',revision:activeEntry?activeEntry.revision:entry.revision,source:result.source};
  }
  function activeLocalEntry(s,entry,local,result,purpose) {return local&&result.source==='继承全局用途'?s.applied['global:'+purpose]||entry:entry;}
  function localContext(s,app) {
    const gid=s.scope==='game'?currentGameId(app):null,o=gid&&override(s.draft,gid,'translation',false),b=editableBinding(s,app),canEdit=!gid||!!o&&o.mode==='override';
    return {binding:b||formBinding(s,app),editable:!!b&&canEdit,gameId:gid,scopeLabel:gid?gameName(app,gid)+' · 游戏专用':'全局默认',scopeHTML:scopeBar(s,app)};
  }
  function translationExtras(s,b,isInherited) {
    const items=Object.values(s.draft.models).map(x=>[x.id,x.name]).concat(window.LunaLocalTranslation.options(s,'translation'));
    return section('并列翻译结果',row('附加引擎',items.filter(([id])=>id!==b.modelId).map(([id,name])=>'<label class="ls-check ls-spaced"><input type="checkbox" data-settings-field="extraModel" data-model="'+esc(id)+'" '+((b.extraModels||[]).includes(id)?'checked ':'')+(isInherited?'disabled':'')+'>'+esc(name)+'</label>').join(''),'按所选顺序显示，各自独立进度、来源与错误；不是自动回退。'));
  }
  function readingOptions(s) {return {segmentation:s.general.segmentation,furigana:s.general.furigana,pos:s.general.pos!==false,fontScale:Number(s.preferences.fontScale)||1,preset:s.preferences.preset,appearance:s.preferences.appearance};}
  function dirty(s,message) {s.dirty=true;s.notice=message||'更改保留在编辑草稿中，正在使用的配置保持不变。';s.noticeKind='info';}
  function formBinding(s,app) {return resolved(s.draft,s.purpose,s.scope==='game'?currentGameId(app):null).binding;}
  function editableBinding(s,app) {if(s.scope==='game'){const o=override(s.draft,currentGameId(app),s.purpose,true);if(o.mode==='inherit')return null;return o.backup;}return s.draft.bindings[s.purpose];}
  function button(label,action,opts) {opts=opts||{};return '<button type="button" class="ls-button '+(opts.primary?'ls-primary ':'')+(opts.danger?'ls-danger ':'')+'" data-settings-action="'+action+'" '+(opts.value?'data-value="'+esc(opts.value)+'" ':'')+(opts.disabled?'disabled ':'')+'>'+label+'</button>';}
  function input(field,value,options) {options=options||{};return '<input '+(options.type?'type="'+options.type+'"':'type="text"')+' data-settings-field="'+field+'" value="'+esc(value)+'" '+(options.min!=null?'min="'+options.min+'" ':'')+(options.max!=null?'max="'+options.max+'" ':'')+(options.disabled?'disabled ':'')+(options.placeholder?'placeholder="'+esc(options.placeholder)+'" ':'')+' aria-label="'+esc(options.label||field)+'">';}
  function select(field,value,items,disabled) {return '<select data-settings-field="'+field+'" aria-label="'+esc(field)+'" '+(disabled?'disabled':'')+'>'+items.map(x=>'<option value="'+esc(x[0])+'" '+(String(x[0])===String(value)?'selected':'')+'>'+esc(x[1])+'</option>').join('')+'</select>';}
  function row(label,control,help) {return '<div class="ls-row"><div class="ls-row-label">'+label+'</div><div class="ls-row-value">'+control+(help?'<div class="ls-help">'+help+'</div>':'')+'</div></div>';}
  function section(title,rows,extra) {return '<section class="ls-section"><h3>'+title+'</h3><div class="ls-form">'+rows+'</div>'+(extra?'<p class="ls-section-note">'+extra+'</p>':'')+'</section>';}
  function check(field,value,label,disabled) {return '<label class="ls-check"><input type="checkbox" data-settings-field="'+field+'" '+(value?'checked ':'')+(disabled?'disabled ':'')+'><span>'+label+'</span></label>';}
  function modelOptions(s,includeEmpty,purpose) {return (includeEmpty?[['','选择配置…']]:[]).concat(Object.values(s.draft.models).filter(m=>!purpose||purpose==='translation'||s.draft.connections[m.connectionId]&&s.draft.connections[m.connectionId].protocol!=='traditional').map(m=>[m.id,m.name])).concat(window.LunaLocalTranslation.options(s,purpose));}
  function affected(s,connectionId) {
    const modelNames=new Set(),labels=new Set();
    function collect(d,b,label){const ids=[b.modelId].concat(b.extraModels||[]);for(const id of ids){const m=d.models[id];if(m&&m.connectionId===connectionId){modelNames.add(m.name);labels.add(label);}}}
    Object.entries(s.draft.models).forEach(([,m])=>{if(m.connectionId===connectionId)modelNames.add(m.name);});
    Object.entries(s.draft.bindings).forEach(([p,b])=>collect(s.draft,b,'草稿 · 全局 · '+names[p]));
    Object.entries(s.draft.overrides).forEach(([g,purposes])=>Object.entries(purposes).forEach(([p,o])=>{if(o.mode==='override')collect(s.draft,o.backup,'草稿 · '+g+' · '+names[p]);}));
    Object.entries(s.applied).forEach(([key,entry])=>{const parts=key.split(':'),g=parts[0],p=parts[1];let snapshot=entry.snapshot,result=resolved(snapshot,p,g==='global'?null:g);if(g!=='global'&&result.source==='继承全局用途'&&s.applied['global:'+p]){snapshot=s.applied['global:'+p].snapshot;result=resolved(snapshot,p,null);}collect(snapshot,result.binding,'已应用 · '+(g==='global'?'全局':g)+' · '+names[p]);});
    return {models:Array.from(modelNames),purposes:Array.from(labels)};
  }
  function scopeBar(s,app) {
    const gid=currentGameId(app),o=override(s.draft,gid,s.purpose,false);
    return '<div class="ls-scope"><span>作用范围</span>'+select('scope',s.scope,[['global','全局默认']].concat(gid?[['game',gameName(app,gid)+' · 专用设置']]:[]))+(s.scope==='game'?select('overrideMode',o?o.mode:'inherit',[['inherit','继承全局'],['override','使用专用设置'],['off','仅此游戏关闭']]):'<span class="ls-scope-hint">'+(gid?'用于没有专用覆盖的游戏':'添加游戏后可配置专用设置')+'</span>')+'</div>';
  }
  function purposeView(s,app) {
    const p=s.purpose,b=formBinding(s,app),m=s.draft.models[b.modelId],c=m&&s.draft.connections[m.connectionId],eff=effective(s,p,s.scope==='game'?currentGameId(app):null),isInherited=s.scope==='game'&&(!override(s.draft,currentGameId(app),p,false)||override(s.draft,currentGameId(app),p,false).mode!=='override');
    const purposeTabs='<div class="ls-purpose-picker" role="group" aria-label="选择用途">'+Object.entries(names).map(([id,name])=>'<button class="'+(id===p?'selected':'')+'" data-settings-action="purpose" data-value="'+id+'">'+name+'</button>').join('')+'</div>';
    if(p==='translation') {const active=window.LunaTranslationServices?.selection(app,s.scope==='game'?currentGameId(app):null),label=active?.mainName||eff.modelName;return purposeTabs+section('当前翻译服务',row('正在使用','<span class="ls-effective">'+esc(label)+'</span>',active?esc(active.source)+' · 已选择版本 v'+active.revision:'读取当前选择，不读取未保存编辑。')+row('更换服务','<button type="button" class="ls-button" data-service-action="open" data-origin="settings">选择翻译服务…</button>','在统一选择器中一次选用；保存配置不会更换来源或开启自动翻译。'))+'<p class="ls-boundary">这里管理连接、模型、模板与检测。句子精读和语境查词分别保留自己的模型配置。</p>'; }
    if(p==='translation'&&window.LunaLocalTranslation.isLocal(b.modelId))return window.LunaLocalTranslation.purposeView(s,app,{binding:b,purposeTabs,scopeHTML:scopeBar(s,app),bindingSelect:select('binding.modelId',b.modelId,modelOptions(s,true,p),isInherited),enabledControl:'<span class="ls-effective">'+esc(eff.status)+'</span>',effective:eff,multiHTML:translationExtras(s,b,isInherited)});
    const bindingSection=section('用途与来源',row('使用配置',select('binding.modelId',b.modelId,modelOptions(s,true,p),isInherited),c?esc(c.name)+' · '+esc(protocolNames[c.protocol])+'；'+(c.protocol==='traditional'?'传统翻译渠道，无模型 ID。':'请求模型 '+esc(m.modelId)):'请选择模型配置。')+row('用途状态',check('binding.enabled',b.enabled,'启用'+names[p],isInherited),'启用需先通过本地校验，保存草稿不会开始请求。')+row(p==='translation'?'翻译语言':'解释语言',select('binding.language',b.language,[['简体中文','日语 → 简体中文'],['English','日语 → English'],['日本語','日语 → 日本語']],isInherited))+(p!=='translation'?row('输出模式',select('binding.outputMode',b.outputMode,[['structured','结构化精读'],['text','文字解释']],isInherited),b.outputMode==='structured'?'应用侧须校验结构及原句引用；HTTP 成功不代表文法可用。':'以自然语言解释，不绘制精确文法高亮。'):row('自动翻译',check('binding.auto',b.auto,'新台词到达时自动翻译',isInherited))));
    const sourceSection=section('输入范围',row('近期上下文',input('binding.historyCount',b.historyCount,{type:'number',min:0,max:20,disabled:isInherited,label:'上下文句数'})+'<span class="ls-inline-unit">句</span>','0 表示只发送当前句。按同一游戏、同一会话隔离。')+row('历史内容',select('binding.context',b.context,[['original','仅原文'],['translation','仅译文'],['both','原文与译文']],isInherited))+row('有效配置','<span class="ls-effective">'+esc(eff.modelName)+' · v'+eff.revision+'</span><span class="ls-badge">'+esc(eff.status)+'</span>','来源：'+esc(eff.source)+'。当前编辑：'+esc(m&&m.name||'未选择')+(s.dirty?'；尚未保存。':'。')));
    let multi='';
    if(p==='translation')multi=translationExtras(s,b,isInherited);
    return purposeTabs+scopeBar(s,app)+bindingSection+sourceSection+multi+'<p class="ls-boundary">内置分词与词典始终可用。精读和语境查词只在你主动发起后使用模型。</p>'+((app.state.windows&&app.state.windows.learning)?button('返回精读窗口','return-learning'):'');
  }
  function connectionView(s) {
    const c=s.draft.connections[s.selectedConnection]||Object.values(s.draft.connections)[0];if(!c)return '<p>尚无连接。</p>';s.selectedConnection=c.id;
    const impact=affected(s,c.id),blocked=s.blockedConnections[c.id];
    return '<div class="ls-resource-bar">'+select('selectedConnection',c.id,Object.values(s.draft.connections).map(x=>[x.id,x.name]))+button('＋ 新增','new-connection')+button('复制','copy-connection')+'</div>'+section('服务连接',row('名称',input('connection.name',c.name,{label:'连接名称'}))+row('协议',select('connection.protocol',c.protocol,Object.entries(protocolNames)))+row('部署位置',select('connection.location',c.location,[['cloud','云端服务'],['local','本机服务'],['remote','远程自托管']]))+row('服务地址',input('connection.endpoint',c.endpoint,{label:'服务地址'}),'<code>'+esc(c.endpoint.replace(/\/$/,''))+(c.protocol==='openai'?'/chat/completions':'')+'</code><br>设计地址；原型不会建立连接。')+row('认证方式',select('connection.auth',c.auth,[['key','API Key 引用'],['none','无需认证']]))+(c.auth==='key'?row('凭据引用',select('connection.credential',c.credential,[['','尚未关联'],['keyref:demo-study-v1','学习服务 · 演示引用 v1'],['keyref:demo-study-v2','学习服务 · 演示引用 v2'],['keyref:demo-microsoft-v1','Microsoft · 演示引用 v1']]),'仅展示安全存储引用。此设计原型不接收真实密钥。'):'')+row('代理',select('connection.proxy',c.proxy,[['inherit','继承系统代理'],['direct','直接连接']])) )+section('连接可用性',row('当前状态','<span class="ls-badge '+(blocked?'ls-warning':'')+'">'+(blocked?'已停用':'可供用途使用')+'</span>'+button(blocked?'恢复连接…':'停用连接…',blocked?'restore-connection':'disable-connection'),'停用立即阻止依赖用途，保留其绑定和原启用意图。')+row('引用关系',esc(impact.models.join('、')||'尚无模型')+'<div class="ls-help">'+esc(impact.purposes.join('；')||'未分配用途')+'</div>')+row('删除连接',button('删除连接…','delete-connection',{danger:true}),'删除前检查引用，不删除阅读历史或其他连接共享的凭据。'));
  }
  function modelView(s) {
    const m=s.draft.models[s.selectedModel]||Object.values(s.draft.models)[0];if(!m)return '<p>尚无模型配置。</p>';s.selectedModel=m.id;const c=s.draft.connections[m.connectionId];
    return '<div class="ls-resource-bar">'+select('selectedModel',m.id,modelOptions(s,false))+button('＋ 新增','new-model')+button('复制','copy-model')+'</div>'+section('模型配置',row('显示名称',input('model.name',m.name,{label:'模型显示名称'}),'只改变可读名称，不替换实际请求 ID。')+row('所属连接',select('model.connectionId',m.connectionId,Object.values(s.draft.connections).map(x=>[x.id,x.name])),c?esc(protocolNames[c.protocol])+' · '+esc(c.endpoint):'选择服务连接。')+(c&&c.protocol==='traditional'?row('模型 ID','<span class="ls-muted">传统翻译接口，无需模型 ID</span>'):row('实际模型 ID',input('model.modelId',m.modelId,{label:'实际模型 ID'})+button('刷新列表','refresh-models'),s.modelListState==='failed'?'<span class="ls-error-text">列表接口不可用（演示）。原选择已保留，你仍可手填 ID 并测试。</span>':'模型枚举不等于用途能力验证。支持直接手填。'))+row('配置用途',select('model.templatePurpose',m.templatePurpose||'',[['','请选择用途'],['translation','翻译'],['grammar','句子精读'],['word','语境查词']]),'用途明确声明；名称不会决定它是否出现在翻译服务列表。')+row('能力声明',select('model.capability',m.capability,[['translation','仅翻译'],['text','文本解释'],['structured','结构化解释']]),'来源：设计样本声明 · 尚未经过真实服务验证。')+row('模型版本','<span>编辑版本 v'+m.version+'</span>','模型没有额外启用开关，由用途选择；同一连接可引用不同模型。'))+section('引用关系',row('全局用途',Object.entries(s.draft.bindings).filter(([,b])=>b.modelId===m.id||(b.extraModels||[]).includes(m.id)).map(([p])=>'<span class="ls-badge">'+names[p]+'</span>').join(' ')||'尚未分配')+row('管理',button('删除模型配置…','delete-model',{danger:true}),'复制会生成独立模板和参数，仍引用同一个连接。'));
  }
  function parameterPreview(s,app) {
    const b=formBinding(s,app),m=s.draft.models[s.selectedModel]||s.draft.models[b.modelId],c=m&&s.draft.connections[m.connectionId];if(!m||!c)return '请选择模型配置。';
    const mode=b.temperatureMode==='inherit'?m.temperatureMode:b.temperatureMode,temp=b.temperatureMode==='inherit'?m.temperature:b.temperature;
    const request={protocol:c.protocol,endpoint:c.endpoint,model:c.protocol==='traditional'?'此渠道不使用模型 ID':m.modelId,purpose:names[s.purpose],source:'放課後、図書館で彼女と待ち合わせをした。',history:Number(b.historyCount)?['昨日は雨だった。'].slice(0,Number(b.historyCount)):[],target_language:b.language};
    if(mode==='set')request.temperature=Number(temp);if(m.maxTokens)request.max_output_tokens=Number(m.maxTokens);
    request.prompt=(b.templateMode==='custom'?b.template:m.template).replace(/{{source}}/g,request.source).replace(/{{word}}/g,'待ち合わせ').replace(/{{history}}/g,request.history.join('\n')).replace(/{{terms}}/g,'').replace(/{{dictionary}}/g,'内置词典：待ち合わせ · 约定会面');
    return JSON.stringify(request,null,2);
  }
  function parametersView(s,app) {
    if(s.engineFamily==='offline')return section('本机离线翻译',row('能力边界','无需模型 ID、提示词或采样参数。<br><button type="button" class="ls-button" data-local-action="manage">管理此本机引擎</button>','精读和语境查词请选择具备解释能力的大模型服务。'));
    const b=formBinding(s,app),m=s.draft.models[s.selectedModel]||s.draft.models[b.modelId];if(!m)return '<p>请先添加模型配置。</p>';s.selectedModel=m.id;
    return '<div class="ls-resource-bar"><span>编辑模型默认</span>'+select('selectedModel',m.id,modelOptions(s,false))+'</div>'+section('模型默认参数',row('温度模式',select('model.temperatureMode',m.temperatureMode,[['inherit','继承适配器默认'],['set','显式设置'],['omit','省略发送 · 服务默认']]),'三种状态；省略会阻断上层显式值，0 则实际发送数字 0。')+(m.temperatureMode==='set'?row('温度值',input('model.temperature',m.temperature,{type:'number',min:0,max:2,label:'模型默认温度'})):'')+row('最大输出长度',input('model.maxTokens',m.maxTokens,{type:'number',min:1,max:16384,label:'最大输出长度'})))+section('模型专用模板',row('提示内容','<textarea data-settings-field="model.template" rows="5" aria-label="模型提示模板">'+esc(m.template)+'</textarea>','变量：{{source}}、{{word}}、{{history}}、{{terms}}、{{dictionary}}。不添加注音；无术语时移除空占位。')+row('模板操作',button('恢复用途默认','reset-template'), '恢复仅修改当前模型的草稿，不改变其他模型副本。'))+section(names[s.purpose]+' · 用途覆盖',row('参数温度',select('binding.temperatureMode',b.temperatureMode,[['inherit','继承模型默认'],['set','显式设置'],['omit','省略发送 · 服务默认']]),'有效优先级：模型默认 → 全局用途 → 游戏覆盖。')+(b.temperatureMode==='set'?row('用途温度',input('binding.temperature',b.temperature,{type:'number',min:0,max:2,label:'用途温度'})):'')+row('模板',select('binding.templateMode',b.templateMode,[['inherit','继承模型模板'],['custom','用途自定义模板']]))+(b.templateMode==='custom'?row('用途提示','<textarea data-settings-field="binding.template" rows="4" aria-label="用途自定义模板">'+esc(b.template)+'</textarea>'):'')+row('请求预览',button('查看有效请求…','preview-request'),'按当前选择与草稿计算；不包含凭据，不发送请求。'));
  }
  function testFingerprint(d,target,gameId,entity) {
    const requestOnly=value=>{if(!value)return null;const clean=clone(value);delete clean.name;delete clean.version;delete clean.enabled;return clean;};
    if(target==='connection')return JSON.stringify(requestOnly(d.connections[entity]));
    if(target==='model'){const m=d.models[entity];return JSON.stringify([requestOnly(m),requestOnly(m&&d.connections[m.connectionId])]);}
    const b=resolved(d,target,gameId).binding,m=b&&d.models[b.modelId];return JSON.stringify([requestOnly(b),requestOnly(m),requestOnly(m&&d.connections[m.connectionId])]);
  }
  function runtimeView(s,app) {
    const b=formBinding(s,app),tests=Object.values(s.tests).reverse();if(s.purpose==='translation'&&!['connection','model'].includes(s.testTarget))s.testTarget='model';
    return section('样例测试',row('测试对象',select('testTarget',s.testTarget,[['connection','当前连接 · '+(s.draft.connections[s.selectedConnection]||{}).name],['model','当前模型 · '+(s.draft.models[s.selectedModel]||{}).name],['translation','实时翻译用途'],['grammar','句子精读用途'],['word','语境查词用途']].filter(item=>s.purpose!=='translation'||['connection','model'].includes(item[0]))))+row('测试样例','<q>放課後、図書館で彼女と待ち合わせをした。</q>','自造日文句；不读取当前游戏字幕，不进入正文历史。')+row('演示返回',select('testOutcome',s.testOutcome,[['pass','符合所选对象的契约'],['auth','认证失败'],['timeout','连接超时'],['structure','结构或原句引用不合格']]),'本地交互样例，不访问真实服务，也不证明模型可用。')+row('执行',button(s.testState==='running'?'取消测试':'运行样例测试',s.testState==='running'?'cancel-test':'test',{primary:true}),s.testState==='running'?'正在处理本地设计样例…':'只更新所选对象的结果；通过不会自动启用用途。'))+section('最近测试记录',tests.length?tests.slice(0,5).map(t=>row(esc(t.label),'<span class="ls-badge '+(t.status==='pass'?'':'ls-warning')+'">'+esc(t.status==='pass'?'样例通过':t.error)+'</span><div class="ls-help">'+esc(t.detail)+' · v'+t.revision+' · '+esc(t.time)+(t.fingerprint!==testFingerprint(s.draft,t.target,t.gameId,t.entity)?' · 编辑已变化，需重测':'')+'</div>')).join(''):row('尚无记录','配置可在未测试时保存；启用时仍检查本地必填和能力冲突。'))+section(names[s.purpose]+' · 运行策略',row('超时',input('binding.timeout',b.timeout,{type:'number',min:5,max:300,label:'用途超时'})+'<span class="ls-inline-unit">秒</span>')+row('并发',input('binding.concurrency',b.concurrency,{type:'number',min:1,max:4,label:'用途并发'}))+row('自动重试',select('binding.retries',b.retries,[[0,'关闭'],[1,'最多 1 次'],[2,'最多 2 次']]))+row('自动回退',select('binding.fallback',b.fallback,[['off','关闭'],['local','显式使用本机模型服务']]),'默认不换服务或付费模型。回退也不得扩大输入范围。')+row('结果缓存',check('binding.cache',b.cache,'复用匹配配置版本的结果'),'模板、方向、术语或输入范围变化会产生新的缓存身份。')+row('用量','<span>未知</span>','服务未返回用量时不推测余额。取消不保证服务不计费。'))+section('多密钥调度',row('密钥池',check('keyPool.enabled',s.keyPool.enabled,'启用演示密钥引用池'),'每个引用独立测试；一个通过不代表整个池。')+row('调度策略',select('keyPool.strategy',s.keyPool.strategy,[['roundrobin','轮询'],['weighted','按错误权重退避']]))+row('凭据引用',s.keyPool.keys.map((k,i)=>'<div class="ls-key-row"><code>'+esc(k.reference)+'</code><span>'+esc(k.status)+'</span>'+button('测试此项','test-key',{value:String(i)})+'</div>').join('')));
  }
  function fragment(markup) {const root=document.createElement('div');root.innerHTML=markup;return root;}
  function advanced(id,title,markup,s) {return '<details class="flat-advanced" data-flat-disclosure="'+id+'" '+(s.flatOpen?.[id]?'open':'')+'><summary>'+title+'</summary>'+markup+'</details>';}
  function withoutRows(root,fields) {fields.forEach(field=>root.querySelector('[data-settings-field="'+field+'"]')?.closest('.ls-row')?.remove());}
  function connectionEditor(s) {
    const root=fragment(connectionView(s)),network=root.querySelector('[data-settings-field="connection.proxy"]')?.closest('.ls-row');
    root.querySelector('.ls-resource-bar')?.remove();
    if(network){network.remove();root.insertAdjacentHTML('beforeend',advanced('connection-network','高级网络设置',network.outerHTML,s));}
    const last=root.querySelectorAll('.ls-section')[1];if(last){last.remove();root.insertAdjacentHTML('beforeend',advanced('connection-management','连接引用与停用 / 删除',last.outerHTML,s));}
    return root.innerHTML;
  }
  function modelEditor(s) {
    const m=s.draft.models[s.selectedModel],c=m&&s.draft.connections[m.connectionId];if(!m||!c)return '';
    if(c.protocol==='traditional')return section('翻译配置',row('配置名称',input('model.name',m.name,{label:'翻译配置名称'})));
    const root=fragment(modelView(s));root.querySelector('.ls-resource-bar')?.remove();
    root.querySelectorAll('.ls-section')[1]?.remove();
    return root.innerHTML;
  }
  function testEditor(s,app) {
    const root=fragment(runtimeView(s,app)),sections=[...root.querySelectorAll('.ls-section')];
    withoutRows(root,['testOutcome']);
    // Test objects still have separate fingerprints and records. Only the simulated
    // outcome control moves to the external review toolbar.
    return (sections[0]?.outerHTML||'')+advanced('test-records','最近测试记录',sections[1]?.outerHTML||'',s);
  }
  function runtimeAdvanced(s,app,id) {
    const root=fragment(runtimeView(s,app)),sections=[...root.querySelectorAll('.ls-section')];
    return advanced(id,'超时、并发、重试与缓存 · '+formBinding(s,app).timeout+'秒 / 并发'+formBinding(s,app).concurrency+' / 重试'+formBinding(s,app).retries+'次',sections[2]?.outerHTML||'',s);
  }
  function learningPurpose(s,app,purpose) {
    const old=s.purpose;s.purpose=purpose;
    const root=fragment(purposeView(s,app));root.querySelector('.ls-purpose-picker')?.remove();
    root.querySelector('[data-settings-action="return-learning"]')?.remove();
    const runtime=runtimeAdvanced(s,app,'runtime-'+purpose);
    const latest=Object.values(s.tests).filter(t=>t.target===purpose).at(-1),testMarkup=section('用途样例测试',row('指定用途',button(s.testState==='running'?'测试处理中…':'测试'+names[purpose],'flat-test-purpose',{value:purpose,disabled:s.testState==='running'}),'只测试此用途，当前游戏正文与其他用途不受影响。')+row('最近结果',latest?(latest.status==='pass'?'样例通过 · ':'失败 · '+latest.error+' · ')+esc(latest.time):'尚未测试'));
    const result='<section class="flat-learning-purpose" data-flat-purpose="'+purpose+'"><h3>'+names[purpose]+'</h3>'+root.innerHTML+runtime+testMarkup+button('应用'+names[purpose]+'配置…','flat-apply-purpose',{value:purpose})+'</section>';
    s.purpose=old;return result;
  }
  function savedServices(s,app) {
    const items=Object.values(s.saved.models||{}).map(m=>{const c=s.saved.connections[m.connectionId];return {id:m.id,name:m.name,family:c?.protocol==='traditional'?'云端翻译':'大模型服务',detail:(c?.location==='local'?'本机 HTTP · ':'')+(names[m.templatePurpose]||'用途待配置'),version:m.version,status:(()=>{const shadow=Object.assign({},s,{draft:s.saved}),errors=validate(shadow,app,m.templatePurpose||'translation',m.id),tests=Object.values(s.tests).filter(t=>(t.target==='model'&&t.entity===m.id||t.target==='connection'&&t.entity===c?.id)&&t.fingerprint===testFingerprint(s.saved,t.target,null,t.entity));return errors.length?'待配置':tests.some(t=>t.status==='fail')?'检测失败':tests.some(t=>t.target==='model'&&t.status==='pass')?'样例通过':'未验证';})()};});
    Object.values(s.saved.localEngines||{}).filter(e=>e.added).forEach(e=>items.push({id:'offline:'+e.id,name:e.name,family:'本机离线翻译',detail:'仅翻译 · '+e.source+' → '+e.target,version:e.version,status:window.LunaLocalTranslation.selectionStatus(s,e).status}));
    return items;
  }
  function translationView(s,app) {
    const local=s.engineFamily==='offline',selected=local?'offline:'+window.LunaLocalTranslation.ensure(s).selected:s.selectedModel;
    const items=savedServices(s,app),draftOnly=Object.values(s.draft.models).filter(m=>!s.saved.models[m.id]).map(m=>({id:m.id,name:m.name,family:'未保存草稿',detail:names[m.templatePurpose]||'用途待配置',version:m.version}));
    Object.values(s.draft.localEngines).filter(e=>e.added&&!s.saved.localEngines?.[e.id]?.added).forEach(e=>draftOnly.push({id:'offline:'+e.id,name:e.name,family:'未保存草稿',detail:'本机离线翻译',version:e.version}));
    const list=(entries)=>entries.map(item=>'<button type="button" class="flat-service-row '+(selected===item.id?'selected':'')+'" data-settings-action="flat-edit-service" data-value="'+esc(item.id)+'" data-flat-service="'+esc(item.id)+'" aria-pressed="'+(selected===item.id)+'"><span><strong>'+esc(item.name)+'</strong><small>'+esc(item.detail)+' · v'+item.version+' · '+esc(item.status||'未保存')+'</small></span><span class="flat-type-label">'+esc(item.family)+'</span><span>编辑 '+icon('chevron-down')+'</span></button>').join('');
    let editor='';
    if(local){const root=fragment(window.LunaLocalTranslation.render(s,app,localContext(s,app)));root.querySelector('.ls-resource-bar')?.remove();root.querySelector('[data-local-action="view-purposes"]')?.remove();root.querySelectorAll('.ls-boundary').forEach(x=>x.remove());editor=root.innerHTML+advanced('local-copy','配置副本','<button type="button" class="ls-button" data-local-action="copy">复制本机配置</button>',s);}
    else {const m=s.draft.models[s.selectedModel];if(m){s.selectedConnection=m.connectionId;s.purpose=m.templatePurpose||'translation';const c=s.draft.connections[m.connectionId];s.testTarget=s.testTarget==='connection'?'connection':'model';
      editor=connectionEditor(s)+modelEditor(s)+testEditor(s,app);
      if(c&&c.protocol!=='traditional')editor+=advanced('model-template','模型模板与参数 · '+(m.temperatureMode==='set'?'温度 '+m.temperature:'服务默认')+' / '+(m.maxTokens||'默认')+' 输出长度',parametersView(s,app),s);
      const runtime=fragment(runtimeView(s,app)),sections=[...runtime.querySelectorAll('.ls-section')];
      editor+=advanced('connection-keys','多密钥与调度 · '+(s.keyPool.enabled?'已启用':'未启用'),sections[3]?.outerHTML||'',s);
      editor+=advanced('service-manage','配置复制与删除',button('复制此模型配置','copy-model')+button('删除此模型配置…','delete-model',{danger:true})+button('复制连接','copy-connection'),s);
    }}
    const oldPurpose=s.purpose;s.purpose='translation';const translationStrategy=scopeBar(s,app)+section('翻译输入范围',row('上下文句数',input('binding.historyCount',formBinding(s,app).historyCount,{type:'number',min:0,max:20,label:'翻译上下文句数'}),'同一游戏和会话隔离；0只发送当前句。')+row('历史内容',select('binding.context',formBinding(s,app).context,[['original','仅原文'],['translation','仅译文'],['both','原文与译文']]))+row('翻译语言',select('binding.language',formBinding(s,app).language,[['简体中文','日语 → 简体中文'],['English','日语 → English'],['日本語','日语 → 日本語']])));const translationRuntime=runtimeAdvanced(s,app,'runtime-translation');s.purpose=oldPurpose;
    const editName=local?s.draft.localEngines[window.LunaLocalTranslation.ensure(s).selected]?.name:s.draft.models[s.selectedModel]?.name;
    return '<div class="ls-content-heading"><span class="ls-heading-icon">'+icon('languages')+'</span><div><h2>翻译服务与学习配置</h2><p>管理已添加的服务；日常更换服务在当前会话或浮窗完成</p></div></div>'+
      '<section class="flat-services" data-flat-section="saved-services"><div class="flat-section-heading"><div><h3>已保存服务</h3><p>保存配置不会更换当前服务或开启自动翻译。</p></div>'+button('添加服务…','flat-add-service')+'</div>'+(items.length?list(items):'<p class="flat-empty">尚无已保存服务。添加服务并保存后，可在日常选择器中选用。</p>')+(draftOnly.length?'<h4>未保存的配置</h4>'+list(draftOnly):'')+'</section>'+
      '<section class="flat-service-editor" id="flat-service-editor" data-flat-section="service-editor" data-flat-editor-type="'+(local?'offline':s.draft.connections[s.selectedConnection]?.protocol==='traditional'?'cloud':'llm')+'"><div class="flat-section-heading"><div><h3>编辑 · '+esc(editName||'服务配置')+'</h3><p>编辑草稿 · 连接、模型与测试属于同一配置</p></div><button type="button" data-service-action="return" class="ls-button">选择已保存服务…</button></div>'+editor+'<div class="flat-save-row">'+button('保存配置','save',{disabled:!s.dirty})+'<span>保存后仍需明确选用；当前句与历史保留。</span></div></section>'+
      '<section class="flat-translation-policy" data-flat-purpose="translation">'+advanced('translation-policy','翻译范围与运行策略',translationStrategy+translationRuntime,s)+'</section>'+
      '<section class="flat-learning" data-flat-section="learning"><div class="flat-section-heading"><div><h3>学习用途</h3><p>句子精读与语境查词独立配置，只在你主动发起时使用。</p></div></div>'+learningPurpose(s,app,'grammar')+learningPurpose(s,app,'word')+'</section>';
  }
  function otherView(s,app) {
    const category=categories.find(c=>c[0]===s.category)||categories[0],head='<div class="ls-content-heading"><span class="ls-heading-icon" aria-hidden="true">'+icon(category[1])+'</span><div><h2>'+category[2]+'</h2><p>首个可用产品版 · 当前配置</p></div></div>';
    if(s.category==='general')return head+section('阅读行为',row('实时输入',check('general.automatic',s.general.automatic,'新台词自动加入当前会话'))+row('历史记录',check('general.history',s.general.history,'保留原文、译文及来源'))+row('启动',check('general.launch',s.general.launch,'登录后打开客户端')))+section('内置分词与词典',row('日语分词',check('general.segmentation',s.general.segmentation,'显示可点击词语'))+row('假名注音',check('general.furigana',s.general.furigana,'在原文上方显示假名'))+row('词性提示',check('general.pos',s.general.pos,'显示轻量词性标记'))+row('基础词典','<span class="ls-badge">随客户端内置</span>','无需下载、选择路径或配置模型。词卡提供原形、读音、词性与中文释义。'))+section('产品范围',row('本阶段','游戏取文、实时翻译、内置分词词典、按需 AI 精读与模型配置。')+row('服务调用','翻译按用途启用；精读与语境查词由你主动发起，点击词语不会自动请求模型。'));
    if(s.category==='input')return head+section('游戏取文',row('输入方式','<span class="ls-badge">LunaHook · CrossOver / Wine</span>','Windows 游戏在所选容器内运行，由独立窄后端提供文本回调。')+row('进程连接',check('input.attach',s.input.attach,'记住上次选择的进程和文本流'))+row('去重',check('input.dedupe',s.input.dedupe,'过滤同一文本流连续重复内容'))+row('合并等待',input('input.merge',s.input.merge,{type:'number',min:0,max:1000,label:'文本合并等待'})+'<span class="ls-inline-unit">毫秒</span>'))+section('其他输入',row('手动文本','从主窗口的“手动文本”粘贴日文，使用相同翻译与学习流程。')+row('OCR 与剪贴板监听','<span class="ls-badge">后续范围</span>','本阶段没有后台捕获或系统权限请求，不提供空启动入口。'));
    if(s.category==='display')return head+section('阅读布局',row('阅读密度',select('preferences.preset',s.preferences.preset,[['comfortable','舒适 · 560 / 18 / 24'],['compact','紧凑 · 500 / 16 / 22']]),'宽度 / 原文字号 / 译文字号，随内容自然增长。')+row('文字缩放',select('preferences.fontScale',s.preferences.fontScale,[[1,'100%'],[1.25,'125%'],[1.5,'150%'],[2,'200%']]))+row('材质',select('preferences.appearance',s.preferences.appearance,[['standard','标准阅读材质'],['reduced','减少透明度 · 实色']])))+section('阅读浮窗',row('译文字号',input('display.fontSize',s.display.fontSize,{type:'number',min:12,max:48,label:'译文字号'})+'<span class="ls-inline-unit">pt</span>')+row('原文字号',input('display.sourceSize',s.display.sourceSize,{type:'number',min:12,max:48,label:'原文字号'})+'<span class="ls-inline-unit">pt</span>')+row('背景不透明度',input('display.opacity',s.display.opacity,{type:'range',min:50,max:100,label:'浮窗背景不透明度'})+'<span>'+s.display.opacity+'%</span>')+row('置顶',check('display.pin',s.display.pin,'游戏在前台时保持浮窗可见')))+section('快捷键',row('显示 / 隐藏浮窗',input('display.hideShortcut',s.display.hideShortcut,{label:'显示隐藏浮窗快捷键'}))+row('暂停 / 继续取文',input('display.pauseShortcut',s.display.pauseShortcut,{label:'暂停继续取文快捷键'}))+row('原文与译文模式',input('display.modeShortcut',s.display.modeShortcut||'',{label:'原译模式快捷键',placeholder:'未设置'}))+row('快捷键应用','<div class="rw-shortcut-buttons"><button type="button" data-rw-action="clear-shortcuts">清除</button><button type="button" data-rw-action="restore-shortcuts">恢复默认</button><button type="button" data-rw-action="apply-shortcuts">应用快捷键</button></div>','编辑与当前生效绑定分开，失败保留原组合。')+row('位置恢复','<button type="button" data-rw-action="reset-hud">恢复浮窗位置</button>','拖动顶部手柄或底部空白区域，隐藏再打开保留位置。'),'快捷键在设计页面内响应；原生全局注册、跨屏与重启恢复另做正式包验收。')+'<p id="rw-shortcut-status" class="ls-boundary" role="status"></p>';
    if(s.category==='processing')return head+section('取文处理',row('连续去重',check('input.dedupe',s.input.dedupe,'过滤同一文本流的连续重复内容'))+row('合并等待',input('input.merge',s.input.merge,{type:'number',min:0,max:1000,label:'文本合并等待'})+'<span class="ls-inline-unit">毫秒</span>')+row('原文保留','取文原始内容与处理后的正文分开；注音、词性色彩不进入翻译输入。'))+section('阶段范围',row('高级替换脚本','后续范围。本阶段不执行用户脚本，也不提供空的运行入口。'));
    if(s.category==='data')return head+section('当前配置档',row('配置档','默认配置 · schema v1','不同配置档隔离连接、模型、用途、草稿与测试。')+row('配置状态','草稿 v'+s.revision+' · '+(s.dirty?'有未保存编辑':'已保存')+'；'+Object.values(s.applied).filter(a=>a.enabled).length+' 个已应用用途')+row('导出',button('导出安全示例…','export'),'仅包含配置与凭据引用，不包含明文密钥、游戏原文或历史。')+row('导入',button('预检导入示例…','import'),'先检查冲突与凭据关联，明确应用后生效，不自动请求。')+row('恢复',button('恢复上一份保存草稿…','restore-saved'),'不覆盖正在使用的版本，不删除历史。'))+(s.importState==='preflight'?section('导入预检',row('检测结果','2 个同名模型；1 个凭据需重新关联；1 个本机 Argos 配置需重新绑定运行器与语言包；无可执行内容，不导入检测 / 测试或启用状态。')+row('处理方式',select('importConflict',s.importConflict||'copy',[['copy','保留现有，以副本导入'],['replace','替换同名草稿']]))+row('下一步',button('应用到草稿','apply-import',{primary:true})+' '+button('取消导入','cancel-import'),'先保留当前草稿备份。导入用途默认关闭，需重新关联凭据。')):'');
    if(s.category==='diagnostics')return head+section('配置诊断',row('连接与模型',Object.keys(s.draft.connections).length+' 个连接 · '+Object.keys(s.draft.models).length+' 个模型配置')+row('用途状态',Object.keys(names).map(p=>'<div class="ls-key-row"><span>'+names[p]+'</span><span>'+esc(effective(s,p,currentGameId(app)).status)+'</span></div>').join(''))+row('设计环境','离线 HTML 原型 · 未调用真实服务 · 未启动 Hook 后端')+row('脱敏诊断',button('生成诊断示例','diagnostic'),s.diagnostic))+'<p class="ls-boundary">游戏连接、实际回调和真实模型能力需要在正式客户端中分别验证。此处不输出虚构通过结果。</p>';
    const descriptions={ocr:'OCR 引擎与屏幕捕获属于后续范围。本阶段通过游戏 Hook 或手动文本输入，不请求屏幕录制权限。',speech:'朗读与语音模型不属于本阶段。当前阅读可使用文字原文、译文与词典。',network:'本阶段的系统代理继承 / 直接连接在“翻译服务 → 连接”中配置。Web API 发布与独立网络高级管理属于后续范围。',integrations:'Anki、插件、Web API 发布和外部自动化属于后续范围。本阶段不提供占位执行按钮。'};
    return head+section('阶段范围',row('当前可用',esc(descriptions[s.category]||'此能力属于后续产品范围。')))+(s.category==='network'?button('前往连接设置','go-connections'):'');
  }
  function render(app) {
    const s=app.state.settings;
    if(s.scope==='game'&&!currentGameId(app))s.scope='global';
    return '<div class="ls-layout"><nav class="ls-sidebar" aria-label="设置分类"><div class="ls-sidebar-top"><button type="button" id="settings-back-main" class="ls-back-main" data-settings-action="back-main"><span class="ls-back-icon" aria-hidden="true">'+icon('arrow-left')+'</span><span>返回主界面</span></button><div class="ls-search"><span class="ls-search-icon" aria-hidden="true">'+icon('search')+'</span><input placeholder="搜索设置" aria-label="搜索设置" data-settings-field="search" value="'+esc(s.search||'')+'"></div></div><div class="ls-sidebar-scroll">'+categories.map(c=>'<button '+(s.search&&!c[2].includes(s.search)?'hidden ':'')+'data-settings-action="category" data-value="'+c[0]+'" class="ls-nav-item '+(s.category===c[0]?'selected':'')+'"><span class="ls-nav-icon" aria-hidden="true">'+icon(c[1])+'</span><span>'+c[2]+'</span></button>').join('')+'<div class="ls-sidebar-foot">个人配置 · 默认<br><span>离线设计样本</span></div></div></nav><div class="ls-main"><div class="ls-scroll">'+(s.category==='translation'?translationView(s,app):otherView(s,app))+'</div><div class="ls-footer"><div class="ls-footer-status" role="status"><span class="'+(s.noticeKind==='error'?'ls-error-text':'')+'">'+esc(s.notice|| (s.dirty?'有未保存更改':'草稿已保存 · 不改变正在使用的配置'))+'</span></div><div class="ls-footer-buttons">'+button('保存配置','save',{disabled:!s.dirty})+(s.category==='translation'?'<button type="button" class="ls-button" data-service-action="return">返回选择翻译服务</button>':'')+'</div></div></div></div>';
  }
  function setRoute(app,purpose) {const s=app.state.settings;s.category='translation';s.tab='purposes';if(names[purpose]){s.purpose=purpose;if(purpose!=='translation')s.engineFamily='llm';s.notice='配置后返回精读窗口，保留原选句，由你明确开始分析。';}else{s.notice='保存与检测不会更换当前翻译服务；从统一选择器更换。';}}
  function rerender(app,resetScroll){const scroll=document.querySelector&&document.querySelector('#settings-content .ls-scroll'),top=scroll?scroll.scrollTop:0,active=document.activeElement,field=active&&active.dataset&&active.dataset.settingsField;app.render();const next=document.querySelector&&document.querySelector('#settings-content .ls-scroll');if(next)next.scrollTop=resetScroll?0:top;if(field&&!resetScroll){const controls=document.querySelectorAll('[data-settings-field]');const replacement=Array.from(controls).find(el=>el.dataset.settingsField===field);if(replacement)replacement.focus({preventScroll:true});}}
  function save(s) {s.revision+=1;s.saved=clone(s.draft);s.dirty=false;s.notice='配置 v'+s.revision+' 已保存；当前服务与自动翻译开关未改变。';s.noticeKind='info';}
  function validate(s,app,purpose,modelId) {
    const b=resolved(s.draft,purpose,s.scope==='game'?currentGameId(app):null).binding,m=s.draft.models[modelId||b.modelId],c=m&&s.draft.connections[m.connectionId],errors=[];
    if(window.LunaLocalTranslation.isLocal(modelId||b.modelId))return window.LunaLocalTranslation.validate(s,purpose,modelId||b.modelId);
    if(!m)errors.push('请为'+names[purpose]+'选择模型配置。');
    if(!c)errors.push('模型没有有效的服务连接。');
    if(c){try{const u=new URL(c.endpoint);if(!/^https?:$/.test(u.protocol))errors.push('服务地址须使用 http 或 https。');if(c.location==='local'&&!['localhost','127.0.0.1','[::1]'].includes(u.hostname))errors.push('本机标签与实际目标不符，请改为远程或云端。');}catch(e){errors.push('请填写有效服务地址。');}if(c.auth==='key'&&!c.credential)errors.push('请关联演示凭据引用。');if(s.blockedConnections[c.id])errors.push('所选连接已停用，请先恢复连接。');}
    if(m&&c){if(c.protocol!=='traditional'&&!m.modelId.trim())errors.push('请填写实际模型 ID。');if(c.protocol==='traditional'&&purpose!=='translation')errors.push('传统翻译连接不支持精读或查词。');if(purpose==='grammar'&&b.outputMode==='structured'&&m.capability!=='structured')errors.push('所选模型未声明结构化能力，请切换文字解释或选择结构化模型。');if(b.templateMode==='inherit'&&m.templatePurpose&&m.templatePurpose!==purpose)errors.push('当前模型模板用于'+names[m.templatePurpose]+'，请为'+names[purpose]+'设置专用模板或恢复该用途默认模板。');const template=b.templateMode==='custom'?b.template:m.template;const variables=template.match(/{{[^}]+}}/g)||[];if(variables.some(v=>!['{{source}}','{{word}}','{{history}}','{{terms}}','{{dictionary}}'].includes(v)))errors.push('模板包含未知变量。');if(!template.includes('{{source}}'))errors.push('模板须包含 {{source}} 原句变量。');if(m.temperatureMode==='set'&&(!Number.isFinite(Number(m.temperature))||Number(m.temperature)<0||Number(m.temperature)>2))errors.push('温度须在 0 至 2 之间，0 是有效显式值。');}
    if(Number(b.historyCount)<0||Number(b.historyCount)>20)errors.push('上下文句数须在 0 至 20 之间。');
    return errors;
  }
  function guardedNavigate(app,change) {const s=app.state.settings;if(!s.dirty){change();rerender(app,true);return;}app.dialog({title:'保留未保存的设置？',description:'已应用的用途不会受到影响。你可以保存编辑草稿后再切换。',body:'<p>放弃只恢复上一次保存的编辑内容，阅读会话继续。</p><button class="ls-button ls-danger" data-settings-action="discard-nav">放弃编辑并切换</button>',confirmLabel:'保存并切换',cancelLabel:'继续编辑',onConfirm:()=>{save(s);change();app.closeDialog();rerender(app,true);}});s.pendingNavigation=change;}
  function exportSafe(s) {
    const draft=clone(s.draft);draft.localEngines=window.LunaLocalTranslation.safeExportResources(s);
    const sanitize=b=>{if(window.LunaLocalTranslation.isLocal(b.modelId))b.enabled=false;};
    Object.values(draft.bindings).forEach(sanitize);Object.values(draft.overrides).forEach(ps=>Object.values(ps).forEach(o=>sanitize(o.backup)));
    return JSON.stringify({schema:1,notice:'离线设计示例；本机资源只含可移植声明，安装目录、Bottle、路径、授权、健康和测试不导出。导入须重新绑定与检查；不会启用。',draft},null,2);
  }
  document.addEventListener('click',function(event){
    const target=event.target.closest('[data-settings-action]');if(!target)return;const app=window.LunaDesign;if(!app)return;const s=app.state.settings,a=target.dataset.settingsAction,v=target.dataset.value;
    if(a==='back-main'){app.closeWindow('settings');app.openWindow('main');const returnTarget=document.getElementById('main-open-settings');if(returnTarget)returnTarget.focus({preventScroll:true});return;}
    if(a==='flat-edit-service'){guardedNavigate(app,()=>{if(v.startsWith('offline:')){s.engineFamily='offline';window.LunaLocalTranslation.ensure(s).selected=v.slice(8);s.purpose='translation';}else{const m=s.draft.models[v];if(m){s.selectedModel=v;s.selectedConnection=m.connectionId;s.engineFamily=s.draft.connections[m.connectionId]?.protocol==='traditional'?'cloud':'llm';s.purpose=m.templatePurpose||'translation';}}});document.getElementById('flat-service-editor')?.scrollIntoView({block:'start'});return;}
    if(a==='flat-test-purpose'){s.purpose=v;s.testTarget=v;const control=document.createElement('button');control.dataset.settingsAction='test';document.body.append(control);control.click();control.remove();return;}
    if(a==='flat-apply-purpose'){s.purpose=v;const control=document.createElement('button');control.dataset.settingsAction='apply';document.body.append(control);control.click();control.remove();return;}
    if(a==='flat-add-service'){app.dialog({title:'添加服务',description:'选择配置类型；添加与保存不会更换当前翻译服务。',body:'<div class="flat-add-choices"><button data-flat-add="cloud">云端翻译<span>传统文本翻译接口</span></button><button data-flat-add="llm">大模型服务<span>云端、本机或远程 HTTP</span></button><button data-flat-add="offline">本机离线翻译<span>系统翻译或已有本机资源</span></button></div>',confirmLabel:'关闭',cancelLabel:'取消',onConfirm:()=>app.closeDialog()});return;}
    if(a==='engine-family'){guardedNavigate(app,()=>{s.engineFamily=v;if(v==='offline'){s.purpose='translation';}else{s.tab='connections';s.selectedConnection=v==='cloud'?'microsoft':'cloud';s.selectedModel=v==='cloud'?'traditional':'study';}});return;}
    if(a==='category'||a==='tab'||a==='purpose'){guardedNavigate(app,()=>{if(a==='category')s.category=v;if(a==='tab')s.tab=v;if(a==='purpose')s.purpose=v;});return;}
    if(a==='discard-nav'){s.draft=clone(s.saved);s.dirty=false;const change=s.pendingNavigation;s.pendingNavigation=null;if(change)change();app.closeDialog();rerender(app);return;}
    if(a==='save'){save(s);rerender(app);return;}
    if(a==='apply'){
      if(s.purpose==='translation'){window.LunaTranslationServices?.open({origin:'settings',scope:s.scope});return;}
      const localBinding=formBinding(s,app);if(window.LunaLocalTranslation.isLocal(localBinding.modelId)){window.LunaLocalTranslation.apply(s,app,localBinding.modelId,localContext(s,app));return;}
      const errors=validate(s,app,s.purpose);if(errors.length){s.notice=errors.join(' ');s.noticeKind='error';rerender(app);return;}
      const p=s.purpose,b=formBinding(s,app),m=s.draft.models[b.modelId],c=s.draft.connections[m.connectionId],gid=s.scope==='game'?currentGameId(app):null;
      const o=gid&&override(s.draft,gid,p,false);
      app.dialog({title:'应用'+names[p]+'配置？',description:'仅将当前版本应用到此用途；其他用途继续使用各自的已应用版本。',body:'<div class="ls-dialog-summary"><p><strong>'+esc(m.name)+'</strong> · '+esc(c.name)+'</p><p>范围：'+esc(gid?gameName(app,gid):'全局默认')+' · '+(o&&o.mode==='off'?'此游戏关闭':b.enabled?'应用并启用':'保持未启用')+'</p><p>输入：当前句'+(Number(b.historyCount)?' + 最多 '+Number(b.historyCount)+' 句同会话上下文':'，无历史')+'</p><p>真实服务能力尚未验证。旧排队及在途请求将取消；原文与内置词典继续。</p></div>',confirmLabel:b.enabled?'应用并启用':'应用此用途',cancelLabel:'取消',onConfirm:()=>{if(s.dirty)save(s);s.applied[(gid||'global')+':'+p]={snapshot:clone(s.draft),revision:s.revision,enabled:b.enabled,source:gid?'游戏专用':'全局用途',savedAt:new Date().toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit'})};s.notice=names[p]+'已应用草稿 v'+s.revision+'；'+(b.enabled?'未验证的演示配置已启用。':'用途保持关闭。');app.closeDialog();rerender(app);}});return;
    }
    if(a==='new-connection'||a==='copy-connection'){const existing=s.draft.connections[s.selectedConnection];const id='connection-'+Date.now();s.draft.connections[id]=a==='copy-connection'?Object.assign(clone(existing),{id,name:existing.name+' 副本',credential:'',version:1}):{id,name:'未命名连接',protocol:'openai',location:'cloud',endpoint:'',auth:'key',credential:'',proxy:'inherit',enabled:true,version:1};s.selectedConnection=id;dirty(s,a==='copy-connection'?'已复制连接。新副本不自动关联凭据，也不分配用途。':'新连接仅在草稿中，不会接收台词。');}
    else if(a==='new-model'||a==='copy-model'){const existing=s.draft.models[s.selectedModel],id='model-'+Date.now();s.draft.models[id]=a==='copy-model'?Object.assign(clone(existing),{id,name:existing.name+' 副本',version:1}):{id,name:'未命名模型',connectionId:s.selectedConnection||'cloud',modelId:'',capability:'text',outputMode:'text',template:defaultTemplate.translation,templatePurpose:'translation',temperatureMode:'inherit',temperature:'',maxTokens:'1024',version:1};s.selectedModel=id;dirty(s,'模型副本独立保存模板与参数，共享连接引用仍可识别。');}
    else if(a==='refresh-models'){s.modelListState='failed';s.notice='模型列表接口不可用（演示）；选择保持不变，可直接编辑实际模型 ID。';}
    else if(a==='reset-template'){s.draft.models[s.selectedModel].template=defaultTemplate[s.purpose];s.draft.models[s.selectedModel].templatePurpose=s.purpose;dirty(s);}
    else if(a==='preview-request'){app.dialog({title:'有效请求预览 · 编辑草稿',description:'以下内容根据所选模型和用途计算；不包含凭据，不发送网络请求。',body:'<pre class="ls-request">'+esc(parameterPreview(s,app))+'</pre>',confirmLabel:'完成',cancelLabel:'关闭',onConfirm:()=>app.closeDialog()});return;}
    else if(a==='disable-connection'||a==='restore-connection'){const c=s.draft.connections[s.selectedConnection],i=affected(s,c.id),disable=a==='disable-connection';app.dialog({title:disable?'停用共享连接？':'恢复共享连接？',description:c.name,body:'<p>依赖模型：'+esc(i.models.join('、')||'无')+'</p><p>受影响用途：'+esc(i.purposes.join('；')||'无')+'</p><p>'+ (disable?'立即停止新请求并取消在途任务；绑定、历史和原启用意图保留。':'明确恢复后，各用途继续使用各自已应用的版本，不使用未应用草稿。')+'</p>',confirmLabel:disable?'停用连接':'恢复所列用途',cancelLabel:'取消',onConfirm:()=>{s.blockedConnections[c.id]=disable;s.notice=disable?'连接已停用，依赖用途已阻止；原文和词典继续。':'连接已恢复，原已应用用途可继续使用。';app.closeDialog();rerender(app);}});return;}
    else if(a==='delete-connection'||a==='delete-model'){const isConnection=a==='delete-connection',id=isConnection?s.selectedConnection:s.selectedModel,modelIds=isConnection?Object.values(s.draft.models).filter(m=>m.connectionId===id).map(m=>m.id):[id],i=isConnection?affected(s,id):{models:[(s.draft.models[id]||{}).name],purposes:Object.entries(s.draft.bindings).filter(([,b])=>modelIds.includes(b.modelId)).map(([p])=>names[p])};app.dialog({title:isConnection?'删除连接与依赖模型？':'删除模型配置？',description:'引用用途将被明确停用，历史与其他凭据保留。',body:'<p>模型：'+esc(i.models.join('、'))+'</p><p>用途：'+esc(i.purposes.join('；')||'无用途引用')+'</p><p>此操作会删除编辑草稿中的资源；已应用用途也停止使用，避免悬空运行。</p>',confirmLabel:'停用引用并删除',cancelLabel:'取消',onConfirm:()=>{Object.values(s.draft.bindings).forEach(b=>{if(modelIds.includes(b.modelId)){b.enabled=false;b.modelId='';}b.extraModels=(b.extraModels||[]).filter(x=>!modelIds.includes(x));});Object.values(s.draft.overrides).forEach(ps=>Object.values(ps).forEach(o=>{if(modelIds.includes(o.backup.modelId)){o.backup.enabled=false;o.backup.modelId='';}}));Object.values(s.applied).forEach(entry=>{Object.values(entry.snapshot.bindings).forEach(b=>{if(modelIds.includes(b.modelId))b.enabled=false;b.extraModels=(b.extraModels||[]).filter(x=>!modelIds.includes(x));});Object.values(entry.snapshot.overrides).forEach(ps=>Object.values(ps).forEach(o=>{if(modelIds.includes(o.backup.modelId))o.backup.enabled=false;}));});if(!s.revokedServices)s.revokedServices={};modelIds.forEach(mid=>{s.revokedServices[mid]=true;delete s.draft.models[mid];});if(isConnection){s.blockedConnections[id]=true;delete s.draft.connections[id];}s.selectedConnection=Object.keys(s.draft.connections)[0]||'';s.selectedModel=Object.keys(s.draft.models)[0]||'';dirty(s,'资源已从草稿删除，引用用途已停用；历史保留。');app.closeDialog();rerender(app);}});return;}
    else if(a==='test'){if(s.testTarget==='translation'&&window.LunaLocalTranslation.isLocal(formBinding(s,app).modelId)){s.engineFamily='offline';window.LunaLocalTranslation.ensure(s).selected=formBinding(s,app).modelId.slice(8);s.notice='本机引擎请分别进行可用性检查与样句翻译；无需 HTTP 认证测试。';rerender(app);return;}const targetType=s.testTarget,p=names[targetType]?targetType:s.purpose,errs=validate(s,app,p,targetType==='model'?s.selectedModel:undefined);if(targetType==='connection'){const currentConnection=s.draft.connections[s.selectedConnection];errs.length=0;try{const parsed=new URL(currentConnection.endpoint);if(!/^https?:$/.test(parsed.protocol))errs.push('连接地址须为 http 或 https。');}catch(e){errs.push('请先填写有效服务地址。');}if(currentConnection.auth==='key'&&!currentConnection.credential)errs.push('请先关联演示凭据引用。');}if(errs.length){s.notice=errs.join(' ');s.noticeKind='error';rerender(app);return;}s.testState='running';s.testToken=(s.testToken||0)+1;const token=s.testToken,testVersion=s.revision,outcome=s.testOutcome,entity=targetType==='connection'?s.selectedConnection:targetType==='model'?s.selectedModel:resolved(s.draft,p,s.scope==='game'?currentGameId(app):null).binding.modelId,gameId=s.scope==='game'?currentGameId(app):null,fingerprint=testFingerprint(s.draft,targetType,gameId,entity),testConnection=clone(s.draft.connections[s.selectedConnection]),testModel=clone(s.draft.models[s.selectedModel]);rerender(app);setTimeout(()=>{if(s.testToken!==token)return;s.testState='idle';const c=testConnection,m=testModel;s.tests[targetType+':'+entity]={target:targetType,entity,gameId,fingerprint,revision:testVersion,status:outcome==='pass'?'pass':'fail',label:targetType==='connection'?'连接 · '+(c&&c.name):targetType==='model'?'模型 · '+(m&&m.name):names[targetType],error:outcome==='auth'?'认证失败':outcome==='timeout'?'连接超时':'结构 / 原句引用不合格',detail:'自造样例 · '+(targetType==='connection'?'仅连接与认证范围':targetType==='model'?'仅指定模型文本请求':names[targetType]+'用途契约')+' · 凭据范围：所选单个引用',time:new Date().toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit'})};s.notice='只更新了“'+(names[targetType]||targetType)+'”样例记录，其他对象状态不变。';rerender(app);},600);return;}
    else if(a==='cancel-test'){s.testToken=(s.testToken||0)+1;s.testState='idle';s.notice='样例测试已取消，草稿保留。';}
    else if(a==='test-key'){const key=s.keyPool.keys[Number(v)];key.status='样例通过 · 仅此引用';s.notice='只更新 '+key.reference+' 的样例状态，未测引用保持原状态。';}
    else if(a==='export'){const json=exportSafe(s);app.dialog({title:'安全导出预览',description:'本机只导出可移植声明；不含安装路径、Bottle、授权、健康和测试；导入须重绑。',body:'<pre class="ls-request">'+esc(json)+'</pre>',confirmLabel:'下载示例 JSON',cancelLabel:'关闭',onConfirm:()=>{const url=URL.createObjectURL(new Blob([json],{type:'application/json'})),link=document.createElement('a');link.href=url;link.download='luna-design-config-example.json';link.click();URL.revokeObjectURL(url);app.closeDialog();}});return;}
    else if(a==='import'){s.importState='preflight';}
    else if(a==='cancel-import'){s.importState='idle';}
    else if(a==='apply-import'){s.importBackup=clone(s.draft);const id='import-study-'+Date.now();s.draft.models[id]=Object.assign(clone(s.draft.models.study||Object.values(s.draft.models)[0]),{id,name:'导入模型 · 待关联'});window.LunaLocalTranslation.importExample(s);s.importState='idle';dirty(s,'示例已导入独立草稿，旧配置已备份；本机目录与语言包需重新绑定、检查和测试，用途未启用。');}
    else if(a==='restore-saved'){app.dialog({title:'恢复上次保存的编辑草稿？',description:'不会更改正在使用的版本或阅读历史。',body:'<p>当前未保存编辑将被替换。</p>',confirmLabel:'恢复草稿',cancelLabel:'取消',onConfirm:()=>{s.draft=clone(s.saved);s.dirty=false;s.notice='已恢复保存的草稿；已应用版本保持不变。';app.closeDialog();rerender(app);}});return;}
    else if(a==='diagnostic'){s.diagnostic='诊断示例已生成：3 层配置关系、脱敏引用与用途版本；不含台词和密钥。';}
    else if(a==='go-connections'){s.category='translation';s.tab='connections';}
    else if(a==='return-learning'){app.openWindow('learning');app.closeWindow('settings');return;}
    rerender(app);
  });
  function updateEditingFooter(s) {
    const message=document.querySelector('#settings-content .ls-footer-status span');
    if(message){message.textContent=s.notice||'有未保存更改';message.classList.toggle('ls-error-text',s.noticeKind==='error');}
    document.querySelectorAll('#settings-content [data-settings-action="save"]').forEach(button=>{button.disabled=!s.dirty;});
  }
  function handleField(event) {
    const el=event.target.closest('[data-settings-field]');if(!el)return;const app=window.LunaDesign;if(!app)return;const s=app.state.settings,key=el.dataset.settingsField,value=el.type==='checkbox'?el.checked:el.value;
    const textEditing=el.tagName==='TEXTAREA'||el.tagName==='INPUT'&&['text','search','number'].includes(el.type);
    if(event.type==='input'&&!textEditing)return;
    // Keep the focused control and the next click target alive during edits and blur.
    // The input event already commits the field value; blur must not rebuild the form.
    if(event.type==='change'&&textEditing)return;
    if(key==='search'){s.search=value;document.querySelectorAll('#settings-content .ls-nav-item').forEach(button=>{button.hidden=!button.textContent.includes(value);});return;}
    if(['selectedConnection','selectedModel','scope'].includes(key)){const old=s[key];s[key]=old;guardedNavigate(app,()=>{s[key]=value;});return;}
    if(['testTarget','testOutcome','importConflict'].includes(key)){s[key]=value;rerender(app);return;}
    if(key==='overrideMode'){const o=override(s.draft,currentGameId(app),s.purpose,true);if(value==='override'&&!o.backup)o.backup=clone(s.draft.bindings[s.purpose]);o.mode=value;dirty(s,value==='inherit'?'重新继承全局，原专用设置备份保留。':'游戏专用设置保留为独立草稿。');rerender(app);return;}
    const parts=key.split('.');let resource;
    if(parts[0]==='connection')resource=s.draft.connections[s.selectedConnection];
    else if(parts[0]==='model')resource=s.draft.models[s.selectedModel];
    else if(parts[0]==='binding')resource=editableBinding(s,app);
    else if(parts[0]==='keyPool')resource=s.keyPool;
    else if(['general','display','input','preferences'].includes(parts[0]))resource=s[parts[0]];
    if(key==='extraModel'){const b=editableBinding(s,app);if(b){const list=b.extraModels||[];b.extraModels=value?Array.from(new Set(list.concat(el.dataset.model))):list.filter(x=>x!==el.dataset.model);dirty(s);}}
    else if(resource){resource[parts[1]]=value;if(parts[0]==='binding'&&parts[1]==='modelId')resource.extraModels=Array.from(new Set(resource.extraModels||[])).filter(id=>id!==value);if(parts[0]==='model'||parts[0]==='connection')resource.version=(resource.version||1)+1;if(['general','display','input','preferences'].includes(parts[0])){s.notice='阅读偏好已更新（设计样本）；模型的已应用版本不变。';if(textEditing)updateEditingFooter(s);else rerender(app);return;}if(parts[0]==='binding'&&parts[1]==='enabled'&&!value){const entry=s.applied[(s.scope==='game'?currentGameId(app):'global')+':'+s.purpose];if(entry){const appliedOverride=s.scope==='game'&&override(entry.snapshot,currentGameId(app),s.purpose,false);if(appliedOverride&&appliedOverride.mode==='override')appliedOverride.backup.enabled=false;else entry.snapshot.bindings[s.purpose].enabled=false;entry.enabled=false;}s.notice='用途已立即停用；仍可保存其他草稿编辑。';}dirty(s,s.notice);}
    if(textEditing)updateEditingFooter(s);else rerender(app);
  }
  document.addEventListener('input',handleField);
  document.addEventListener('change',handleField);
  function serviceStatus(s,ref,snapshot) {
    const d=snapshot||s.saved,m=d.models&&d.models[ref],c=m&&d.connections[m.connectionId];
    const shadow=Object.assign({},s,{draft:clone(d),scope:'global'});
    shadow.draft.bindings=shadow.draft.bindings||{};shadow.draft.overrides=shadow.draft.overrides||{};
    if(m){shadow.draft.models[ref].modelId=String(m.modelId||'');shadow.draft.models[ref].template=String(m.template||'');}
    shadow.draft.bindings.translation=Object.assign({},shadow.draft.bindings.translation,{modelId:ref,templateMode:'inherit',historyCount:0});
    const errors=validate(shadow,{state:{selectedGame:null,session:{gameId:null}}},'translation',ref);
    if(s.revokedServices&&s.revokedServices[ref])errors.push('此模型配置已明确删除；当前选择已停用，历史来源保留。');
    if(m&&!String(m.name||'').trim())errors.push('请填写服务配置名称。');
    if(c&&!protocolNames[c.protocol])errors.push('服务协议尚不受支持。');
    if(c&&!['key','none'].includes(c.auth))errors.push('请声明认证方式。');
    if(c&&c.protocol==='traditional'&&c.auth!=='key')errors.push('Microsoft 文本翻译需要订阅 Key 引用，不能选择无需认证。');
    if(!m||m.templatePurpose!=='translation')errors.push('此配置未声明翻译用途。');
    if(m&&!['translation','text','structured'].includes(m.capability))errors.push('此配置能力不支持翻译。');
    if(c&&c.enabled===false)errors.push('连接已停用。');
    const matches=Object.values(s.tests||{}).filter(t=>(t.target==='model'&&t.entity===ref||t.target==='connection'&&c&&t.entity===c.id)&&t.fingerprint===testFingerprint(d,t.target,null,t.entity));
    const failure=matches.find(t=>t.status==='fail');
    if(failure)errors.push(failure.error||'此配置最近检测失败，请修复并重新检测。');
    return {available:!errors.length,status:errors.length?(failure?'检测失败':'待配置'):matches.some(t=>t.target==='model'&&t.status==='pass')?'样例通过':'未验证',reason:errors.join(' '),verification:failure?'failed':matches.some(t=>t.target==='model'&&t.status==='pass')?'passed':'unverified',configVersion:m&&m.version||0};
  }
  window.LunaSettings={initialState,render,effective,setRoute,resetBindings,readingOptions,localContext,preview:parameterPreview,serviceStatus};
})();
