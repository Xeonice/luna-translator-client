/* Pending offline design revision. All checks and translations are local simulations. */
(function () {
  'use strict';
  const clone = value => JSON.parse(JSON.stringify(value));
  const esc = value => String(value == null ? '' : value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const kinds = {
    apple_translation: {name:'Apple 系统翻译', backend:'platform_native', runtime:'native', note:'使用系统管理的翻译语言包。系统版本、授权和语言方向须检查；此候选未证明产品兼容。'},
    argos_translate: {name:'Argos 专用机器翻译', backend:'local_worker', runtime:'native', note:'专用机器翻译候选运行器，使用用户提供的语言包；并非 Ollama / LM Studio，也未承诺生产兼容或随包分发。'},
    jbeijing: {name:'J北京', backend:'local_worker', runtime:'wine', note:'连接用户已安装的软件与 DLL。须核对版本、位数与 Windows / Wine 环境，不下载或分发商业软件。'},
    kingsoft_fastait: {name:'金山快译', backend:'local_worker', runtime:'wine', note:'连接用户已安装的软件与 DLL。具体版本与词库兼容性未验证，不承诺任意安装版本可用。'}
  };
  const languages = [['ja','日语'],['zh-Hans','简体中文'],['en','English']];
  const statusNames = {idle:'待检查',ready:'就绪',missing_install:'未安装',missing_package:'语言包缺失',unsupported:'版本不支持',failed:'失败',timeout:'超时',expired:'测试过期',cancelled:'已取消',disabled:'禁用',unknown:'能力未知',unimplemented:'运行路线未实现'};
  const outcomes = [['ready','就绪'],['missing_install','未安装'],['missing_package','语言包缺失'],['unsupported','版本不支持'],['failed','执行失败'],['timeout','超时']];
  const samples = [
    {id:'sample-1',source:'放課後、図書館で彼女と待ち合わせをした。',translations:{'zh-Hans':'放学后，我和她约好在图书馆见面。',en:'After school, I arranged to meet her at the library.'}},
    {id:'sample-2',source:'窓の外では、静かに雨が降っていた。',translations:{'zh-Hans':'窗外正静静地下着雨。',en:'Outside the window, rain was falling quietly.'}}
  ];
  function safeExportResources(s) {
    return Object.fromEntries(Object.values(s.draft.localEngines||{}).map(e=>[e.id,{id:e.id,kind:e.kind,backend:e.backend,runtime:e.runtime,source:e.source,target:e.target,packageVersion:/^[A-Za-z0-9_.+@() -]{1,120}$/.test(e.packageVersion)?e.packageVersion:'',route:e.route,pivotConfirmed:false,requiresLocalRebinding:true}]));
  }
  function importExample(s) {
    const id='import-argos-'+Date.now(),e=config('argos_translate',id);
    e.name='导入 Argos · 需重新绑定';e.requiresLocalRebinding=true;s.draft.localEngines[id]=e;
    ensure(s).selected=id;return e;
  }
  function bindingAt(snapshot,scopeKey) {
    const gid=scopeKey.split(':')[0],o=gid!=='global'&&snapshot.overrides[gid]&&snapshot.overrides[gid].translation;
    return o&&o.mode==='override'?o.backup:snapshot.bindings.translation;
  }
  function appliedAt(s,e,context) {
    const a=ensure(s).applied[e.id];if(!a)return null;
    return (a.versions||[a]).filter(version=>version.scopeKey===(context.gameId||'global')+':translation').slice(-1)[0]||null;
  }
  function config(kind,id) {
    return {id:id||kind,added:false,name:kinds[kind].name,kind,backend:kinds[kind].backend,runtime:kinds[kind].runtime,installPath:'',bottle:'',arch:'unknown',packageSource:'',packageVersion:'',source:'ja',target:'zh-Hans',route:'direct_only',pivotConfirmed:false,version:1};
  }
  function resources() {return Object.fromEntries(Object.keys(kinds).map(id=>[id,config(id)]));}
  function ensure(s) {
    if(!s.draft.localEngines)s.draft.localEngines=resources();
    if(!s.localTranslation)s.localTranslation={selected:'apple_translation',checks:{},readiness:{},tests:{},applied:{},enabled:{},running:{},sequence:0,sampleIndexes:{},checkOutcome:'ready',testOutcome:'ready',appleScenario:'unknown',applePair:'supported',argosRoute:'unimplemented',notice:''};
    return s.localTranslation;
  }
  function isLocal(id) {return typeof id==='string'&&id.startsWith('offline:');}
  function engineId(id) {return isLocal(id)?id.slice(8):id;}
  function options(s,purpose) {ensure(s);return purpose==='translation'?Object.values(s.draft.localEngines).map(e=>['offline:'+e.id,e.name+' · 本机离线']):[];}
  function configFingerprint(e) {const value=clone(e);delete value.name;delete value.version;delete value.added;return JSON.stringify(value);}
  function scopeKey(context) {return (context.gameId||'global')+':translation';}
  function enabledKey(key,id) {return key+'|'+id;}
  function runtimeGate(s,e) {
    const u=ensure(s),source='来源：页外设计条件；未读取本机系统或运行器。';
    if(e.kind==='apple_translation') {
      const definitions={
        unknown:['unknown','OS / API 版本未知，无法确认纯传统策略；不可作为非大模型引擎测试、应用或启用。','OS/API 未知 · 传统策略未知'],
        os14:['unsupported','macOS 14：不具备最低 macOS 15 的 Translation API；不可运行。','macOS 14 · API 不可用'],
        os15:['unknown','macOS 15：仅基础 Translation API；尚不能确认可强制传统路线，不可作为纯非大模型引擎执行。','macOS 15 · 基础 API · 传统策略未知'],
        os26:['unknown','macOS 26：headless 与 cancel 能力可用，但尚无已确认的 26.4 lowLatency 传统策略；不能按非大模型路线执行。','macOS 26 · headless / cancel · 传统策略未知'],
        os26_4_unknown:['unknown','macOS 26.4：traditional_strategy_status 未知；不能把版本号当作传统策略已确认。','macOS 26.4 · 策略未知'],
        os26_4_low_latency:['ready','macOS 26.4 lowLatency 传统策略已确认（设计模拟）；仍须按语言对与资源检查。','macOS 26.4 · lowLatency · confirmed_traditional'],
        os26_4_high_fidelity:['unsupported','highFidelity / Apple Intelligence 路线被排除，不能作为本机非大模型引擎。','macOS 26.4 · highFidelity / Apple Intelligence · excluded']
      };
      const value=definitions[u.appleScenario]||definitions.unknown;
      if(value[0]!=='ready')return {allowed:false,status:value[0],reason:value[1],summary:value[2],source};
      if(u.applePair!=='supported')return {allowed:false,status:u.applePair==='unsupported'?'unsupported':'unknown',reason:u.applePair==='unsupported'?'当前源目标语言对不受支持，不能测试、应用或启用。':'当前语言对可用性未知，不能把 API 可用等同于语言对可用。',summary:value[2]+' · 语言对'+(u.applePair==='unsupported'?'不支持':'未知'),source};
      return {allowed:true,status:'ready',reason:value[1],summary:value[2]+' · 语言对支持（模拟）',source};
    }
    if(e.kind==='argos_translate'&&u.argosRoute!=='reviewed_ready')return {allowed:false,status:u.argosRoute==='unimplemented'?'unimplemented':'unknown',reason:u.argosRoute==='unimplemented'?'Argos 候选运行路线尚未实现；语言包存在也不能进行样句测试、应用为可运行配置或启用。':'Argos 分句 / 分词 / 推理运行路线待评审，不能仅凭安装目录认定可运行。',summary:u.argosRoute==='unimplemented'?'候选运行路线未实现':'候选运行路线待评审',source};
    return {allowed:true,status:'ready',reason:e.kind==='argos_translate'?'受审运行路线就绪（设计模拟）；不表示生产 Python 或新增运行库已获批准 / 实现。':'软件版本、位数和运行环境由当前检查单独核验。',summary:e.kind==='argos_translate'?'受审候选路线就绪（设计模拟）':'本机已安装软件路线（待检查）',source};
  }
  function identity(s,e) {
    const gate=runtimeGate(s,e);
    return JSON.stringify([configFingerprint(e),gate.allowed,gate.status,gate.summary]);
  }
  function actualApplied(s,context) {
    const key=scopeKey(context),candidate=s.applied[key];
    if(context.gameId&&candidate){const o=candidate.snapshot.overrides[context.gameId]&&candidate.snapshot.overrides[context.gameId].translation;if(o&&o.mode!=='inherit')return {entry:candidate,key};}
    return {entry:s.applied['global:translation'],key:'global:translation'};
  }
  function applyDisabled(s,app) {const e=current(s);return !e||!contextFor(s,app).editable||!runtimeGate(s,e).allowed;}
  function current(s) {const u=ensure(s);return s.draft.localEngines[u.selected]||Object.values(s.draft.localEngines)[0];}
  function rememberCheck(s,e,record) {
    const u=ensure(s);if(!u.readiness)u.readiness={};if(!u.readiness[e.id])u.readiness[e.id]={};
    u.checks[e.id]=record;u.readiness[e.id][record.fingerprint]=record;
  }
  function readinessFor(s,e) {
    const u=ensure(s),key=identity(s,e),latest=u.checks[e.id];
    return latest&&latest.fingerprint===key?latest:u.readiness&&u.readiness[e.id]&&u.readiness[e.id][key];
  }
  function checkState(s,e) {const record=ensure(s).checks[e.id];return !record?'idle':record.fingerprint===identity(s,e)?record.status:'expired';}
  function sampleFor(s,e) {const sample=samples[ensure(s).sampleIndexes[e.id]||0];return Object.assign({},sample,{source:e.source==='ja'?sample.source:sample.translations[e.source],translations:Object.assign({ja:sample.source},sample.translations)});}
  function testState(s,e) {const record=ensure(s).tests[e.id];return !record?'idle':record.fingerprint===identity(s,e)&&record.sampleId===sampleFor(s,e).id?record.status:'expired';}
  function button(label,action,options) {options=options||{};return '<button type="button" class="ls-button '+(options.primary?'ls-primary':'')+'" data-local-action="'+action+'" '+(options.disabled?'disabled':'')+'>'+label+'</button>';}
  function select(field,value,items) {return '<select id="local-'+field+'" data-local-field="'+field+'" aria-label="'+esc(({selected:'本机引擎配置',kind:'引擎类型',runtime:'运行位置',arch:'运行器位数',source:'源语言',target:'目标语言',route:'翻译路径',sample:'翻译样句'})[field]||field)+'">'+items.map(([id,label])=>'<option value="'+esc(id)+'" '+(String(value)===String(id)?'selected':'')+'>'+esc(label)+'</option>').join('')+'</select>';}
  function input(field,value,label,placeholder) {return '<input type="text" data-local-field="'+field+'" value="'+esc(value)+'" aria-label="'+esc(label)+'" placeholder="'+esc(placeholder||'')+'">';}
  function row(label,body,help) {return '<div class="ls-row"><div class="ls-row-label">'+label+'</div><div class="ls-row-value">'+body+(help?'<div class="ls-help">'+help+'</div>':'')+'</div></div>';}
  function section(title,body) {return '<section class="ls-section"><h3>'+title+'</h3><div class="ls-form">'+body+'</div></section>';}
  function badge(status) {return '<span class="ls-badge '+(['idle','ready','disabled'].includes(status)?'':'ls-warning')+'" data-local-status="'+status+'">'+esc(statusNames[status]||status)+'</span>';}
  function routeLabel(e) {const label=id=>(languages.find(l=>l[0]===id)||[])[1]||id;return label(e.source)+' → '+(e.route==='pivot'?'English → ':'')+label(e.target);}
  function fieldErrors(e) {
    const errors=[];
    if(!e.name.trim())errors.push('请填写配置名称。');
    if(e.source===e.target)errors.push('源语言与目标语言须不同。');
    if(e.route==='pivot'&&[e.source,e.target].includes('en'))errors.push('中转语言不能与源语言或目标语言重复。');
    if(e.kind!=='apple_translation'&&!e.installPath.trim())errors.push('请指定用户已安装的运行器或软件目录。');
    if(e.runtime==='wine'&&!e.bottle.trim())errors.push('请指定运行该软件的 Bottle。');
    if(['jbeijing','kingsoft_fastait'].includes(e.kind)&&e.arch==='unknown')errors.push('请核对软件 / DLL 位数。');
    if(['jbeijing','kingsoft_fastait'].includes(e.kind)&&!e.packageVersion.trim())errors.push('请记录软件 / 词库版本以核对兼容性。');
    if(e.kind==='argos_translate'&&(!e.packageSource.trim()||!e.packageVersion.trim()))errors.push('请记录语言包来源与版本。');
    return errors;
  }
  function validate(s,purpose,ref) {
    if(purpose!=='translation')return ['本机离线引擎仅提供翻译，不能用于句子精读或语境查词。'];
    const e=s.draft.localEngines&&s.draft.localEngines[engineId(ref)];if(!e)return ['本机引擎配置不存在。'];
    const errors=fieldErrors(e),gate=runtimeGate(s,e);
    if(!gate.allowed)errors.push(gate.reason);
    if(checkState(s,e)!=='ready')errors.push('请先完成当前版本的可用性检查；保存草稿不要求就绪。');
    if(e.route==='pivot'&&!e.pivotConfirmed)errors.push('请明确确认中转语言路径。');
    return errors;
  }
  function effective(s,entry,b,source,ref,purpose,key) {
    const id=engineId(ref),e=entry.snapshot.localEngines&&entry.snapshot.localEngines[id],u=ensure(s),versions=u.applied[id]&&(u.applied[id].versions||[u.applied[id]])||[];
    const gate=e&&runtimeGate(s,e),record=e&&versions.find(version=>version.scopeKey===key&&version.fingerprint===identity(s,e)),scopeEnabled=!!u.enabled[enabledKey(key,id)],health=e&&readinessFor(s,e);
    const enabled=purpose==='translation'&&!!e&&!!b.enabled&&scopeEnabled&&!!record&&gate.allowed&&!!health&&health.status==='ready';
    return {enabled,connectionEnabled:purpose==='translation'&&!!e&&scopeEnabled&&!!gate&&gate.allowed&&!!health&&health.status==='ready',modelId:ref,modelName:e?e.name:'本机配置已移除',connectionName:'本机离线翻译',outputMode:'text',historyCount:0,status:purpose!=='translation'?'仅翻译能力，不支持此用途':!e?'未配置':!gate.allowed?gate.reason:!health?'已应用版本需要重新检查':health.status!=='ready'?'已应用资源'+(statusNames[health.status]||health.status):enabled?'本机翻译已启用':statusNames.disabled,revision:entry.revision,source};
  }
  function readiness(s,e) {
    const u=ensure(s),gate=runtimeGate(s,e),status=gate.allowed?checkState(s,e):gate.status,record=u.checks[e.id],running=u.running[e.id];
    return section('可用性检查',row('执行能力','<span data-local-capability="'+(gate.allowed?'available':'blocked')+'">'+esc(gate.summary)+'</span>',''+esc(gate.reason)+'<br>'+esc(gate.source))+row('当前状态','<span id="local-readiness-status">'+badge(status)+'</span>'+(running&&running.type==='check'?'<span class="ls-muted">正在检查…</span>':''),record?esc(record.detail)+'<br>归属：'+esc(e.name)+' · 配置 v'+record.version+' · '+esc(record.at)+(status==='expired'?'；配置已变化，旧结果不能用于启用。':''):'尚未探测系统、运行器、DLL 或语言包。')+row('检查',button('检查可用性','check',{disabled:!!running})+(running&&running.type==='check'?button('取消当前任务','cancel'):''),'只检查当前配置与语言方向，不改变用途，不下载资源。')+row('资源操作',button('查看准备方式','resource-help'),'安装、授权或语言包获取均由你明确处理；不会静默下载或切换为云端。'));
  }
  function sampleView(s,e) {
    const u=ensure(s),sample=sampleFor(s,e),test=u.tests[e.id],running=u.running[e.id],status=testState(s,e);
    return section('样句翻译',row('当前样句',select('sample',u.sampleIndexes[e.id]||0,samples.map((x,i)=>[i,'样句 '+(i+1)]))+'<p class="local-sample-source" lang="'+esc(e.source)+'">'+esc(sample.source)+'</p>','样句只属于此测试，不写入游戏正文或阅读历史。')+row('翻译测试',button('翻译这句','test',{primary:true,disabled:!!running||!runtimeGate(s,e).allowed||checkState(s,e)!=='ready'})+(running&&running.type==='test'?button('取消当前任务','cancel'):'')+button('换一条样句','next-sample'),'与可用性检查独立。换句、修改配置或取消后，旧任务结果不会覆盖新对象。')+row('测试结果','<span id="local-sample-status">'+badge(status)+'</span>'+(test&&test.result?'<p class="local-sample-result">'+esc(test.result)+'</p>':''),test?'设计模拟 · '+esc(test.engineName)+' · '+esc(test.sampleId)+' · 配置 v'+test.version+'<br>'+esc(test.detail)+(status==='expired'?' · 当前配置或样句已变化。':''):'尚无当前样句的翻译结果。'));
  }
  function appliedView(s,e,context,app) {
    const active=window.LunaTranslationServices?.selection(app,context.gameId),isSaved=!!s.saved.localEngines?.[e.id]?.added;
    return section('保存与选用',row('配置状态','<span class="ls-effective">'+esc(e.name)+' · 编辑版本 v'+e.version+'</span>',isSaved?'已有保存版本；本次未保存编辑不参与选择。':'尚未添加并保存；不会出现在日常服务选择器。')+row('当前翻译服务',esc(active?.mainName||'未选择'),'保存、检查与样句不更换当前服务，也不改变自动翻译开关。')+row('选用服务','<button type="button" class="ls-button" data-service-action="return">返回选择翻译服务</button>','完成配置并保存后，从统一选择器选为主服务或附加对照。'));
  }
  function render(s,app,context) {
    const u=ensure(s),e=current(s);if(!e)return '';
    let fields=row('配置名称',input('name',e.name,'本机配置名称'))+row('引擎类型',select('kind',e.kind,Object.entries(kinds).map(([id,v])=>[id,v.name])),esc(kinds[e.kind].note))+row('运行位置',select('runtime',e.runtime,e.kind==='apple_translation'?[['native','当前 Mac · 系统原生']]:e.kind==='argos_translate'?[['native','当前 Mac · 独立运行器'],['windows','Windows · 独立运行器'],['wine','CrossOver / Wine · 独立运行器']]:[['windows','Windows · 已安装软件'],['wine','CrossOver / Wine · 已安装软件']]),e.kind==='apple_translation'?'系统能力与语言资源由 macOS 管理。':'独立本机执行边界；软件 / DLL 故障不会替换原句、内置词典或游戏取文。');
    if(e.kind==='apple_translation')fields+=row('系统与语言包','<span class="ls-muted">'+(checkState(s,e)==='ready'?'设计模拟：系统 / 授权 / 语言方向就绪':'版本、授权、语言包：待检查')+'</span>','首次准备或系统授权须由用户确认；离线可用性按当前系统与语言方向逐项核验。');
    else fields+=row('安装目录',input('installPath',e.installPath,'本机安装目录',e.runtime==='native'?'/Users/…/translation-runner':'C:\\Program Files\\…'),'手动填写现有路径。本设计不读取文件系统，也不会安装软件。')+(e.runtime==='wine'?row('运行环境',input('bottle',e.bottle,'运行 Bottle','选择已安装软件所在的 Bottle'),'路径、Bottle 与目标程序必须属于同一安装。'):'')+(['jbeijing','kingsoft_fastait'].includes(e.kind)?row('软件 / DLL 位数',select('arch',e.arch,[['unknown','尚未核对'],['x86','32 位 · x86'],['x64','64 位 · x64']]),'须匹配隔离运行器；可用版本由实际适配检查决定。'):'')+row(e.kind==='argos_translate'?'语言包来源':'软件 / 词库来源',input('packageSource',e.packageSource,e.kind==='argos_translate'?'语言包来源':'软件词库来源','记录用户安装来源'),'只记录来源，不假定许可、质量、兼容性或可分发权。')+row(e.kind==='argos_translate'?'语言包版本':'软件 / 词库版本',input('packageVersion',e.packageVersion,'资源版本','填写版本或包标识'),'资源或版本变化后，旧检测和测试失效。');
    fields+=row('源语言',select('source',e.source,languages))+row('目标语言',select('target',e.target,languages));
    if(e.kind==='argos_translate')fields+=row('翻译路径',select('route',e.route,[['direct_only','仅使用直达语言对'],['pivot','允许中转 · 需明确确认']]),'缺少直达语言对时明确失败；不自动改用英文中转、其他引擎或云端。')+(e.route==='pivot'?row('中转确认','<span class="ls-effective">'+esc(routeLabel(e))+'</span>'+badge(e.pivotConfirmed?'ready':'idle')+button(e.pivotConfirmed?'重新查看中转路径':'确认此中转路径…','pivot'),'中转可能改变质量和耗时。确认仅对当前来源 / 目标 / 语言包版本生效。'):'');
    return '<div id="local-translation-config" data-local-engine="'+esc(e.id)+'"><div class="ls-resource-bar">'+select('selected',e.id,Object.values(s.draft.localEngines).map(x=>[x.id,x.name]))+(!e.added?button('添加此配置','add-current'):'')+button('新增配置','new')+button('复制配置','copy')+'</div>'+section('本机离线翻译配置',fields)+readiness(s,e)+sampleView(s,e)+appliedView(s,e,context,app)+'<p class="ls-boundary">仅翻译用途 · '+Object.keys(s.draft.localEngines).length+' 个独立本机配置。Ollama / LM Studio 请在“大模型服务”中配置。</p>'+button('查看用途与并列结果','view-purposes')+'<p id="local-edit-freshness" class="ls-help" role="status"></p></div>';
  }
  function purposeView(s,app,context) {
    const id=engineId(context.binding.modelId),draft=s.draft.localEngines[id],scope=contextFor(s,app),actual=actualApplied(s,scope),live=actual.entry&&bindingAt(actual.entry.snapshot,actual.key),appliedEngine=live&&isLocal(live.modelId)&&actual.entry.snapshot.localEngines[engineId(live.modelId)];
    return context.purposeTabs+context.scopeHTML+section('用途与来源',row('编辑配置',context.bindingSelect,'本机离线翻译 · 仅翻译能力；无需 API Key、模型 ID 或提示词。')+row('草稿语言方向',draft?esc(routeLabel(draft))+' · 草稿 v'+draft.version:'配置不存在','仅表示正在编辑的配置，不覆盖已应用来源。')+row('已应用来源','<span data-local-effective-source>'+esc(context.effective.modelName)+' · 用途 v'+context.effective.revision+'</span>',esc(context.effective.source))+row('已应用语言方向',appliedEngine?'<span data-local-effective-direction>'+esc(routeLabel(appliedEngine))+' · 引擎 v'+appliedEngine.version+'</span>':'当前用途尚未应用此本机配置。','读取当前作用范围的实际已应用快照；不使用其他游戏或未应用草稿。')+row('用途状态',context.enabledControl,'保存与应用均不会自动启用本机引擎。')+row('引擎配置',button('管理此本机引擎','manage'),'精读和语境查词仍使用具备解释能力的模型，内置词典保持独立。'))+context.multiHTML;
  }
  function contextFor(s,app) {return window.LunaSettings.localContext(s,app);}
  function renderApp(app) {const scroll=document.querySelector('#settings-content .ls-scroll'),top=scroll&&scroll.scrollTop;app.render();const next=document.querySelector('#settings-content .ls-scroll');if(next)next.scrollTop=top||0;}
  function notice(s,text,error) {s.notice=text;s.noticeKind=error?'error':'info';}
  function saveDraft(s) {s.revision+=1;s.saved=clone(s.draft);s.dirty=false;}
  function cancel(s,e,reason) {
    const u=ensure(s),job=u.running[e.id];if(!job)return;
    delete u.running[e.id];
    const record={fingerprint:job.fingerprint,status:'cancelled',detail:reason||'用户取消；旧任务回执不会进入当前结果。',version:job.version,at:new Date().toLocaleTimeString('zh-CN'),sampleId:job.sampleId,engineName:e.name};
    if(job.type==='check')rememberCheck(s,e,record);else u.tests[e.id]=record;
  }
  function run(s,app,e,type) {
    const u=ensure(s),gate=runtimeGate(s,e),errors=fieldErrors(e);
    if(!gate.allowed){if(type==='check')rememberCheck(s,e,{fingerprint:identity(s,e),status:gate.status,detail:gate.reason+' '+gate.source,version:e.version,at:new Date().toLocaleTimeString('zh-CN')});notice(s,gate.reason,true);renderApp(app);return;}
    if(errors.length){notice(s,errors.join(' '),true);renderApp(app);return;}
    if(type==='test'&&(checkState(s,e)!=='ready'||e.route==='pivot'&&!e.pivotConfirmed)){notice(s,'请先完成当前版本检查，并确认所需中转路径。',true);renderApp(app);return;}
    cancel(s,e,'被当前配置的新任务替代。');
    const sample=sampleFor(s,e),job={id:++u.sequence,type,scopeKey:scopeKey(contextFor(s,app)),engineId:e.id,fingerprint:identity(s,e),version:e.version,sampleId:sample.id,outcome:type==='check'?u.checkOutcome:u.testOutcome};
    u.running[e.id]=job;notice(s,'正在执行设计模拟；不访问网络、不下载、不探测本机。');renderApp(app);
    setTimeout(()=>{
      const live=s.draft.localEngines[e.id];
      if(u.running[e.id]!==job||!live||identity(s,live)!==job.fingerprint||type==='test'&&sampleFor(s,live).id!==job.sampleId)return;
      delete u.running[e.id];
      const status=job.outcome,detail=status==='ready'?(type==='check'?'设计样本：当前引擎、资源与语言方向就绪。实际兼容性尚未验证。':'设计样句完成；不代表真实运行器或翻译质量通过。'):({missing_install:'设计模拟：未找到所选安装；请检查安装目录和运行环境。',missing_package:'设计模拟：缺少该语言方向所需语言包；未启动下载或中转。',unsupported:'设计模拟：系统、运行器或软件版本不受支持；保留配置。',failed:'设计模拟：执行失败；原文与其他引擎继续。',timeout:'设计模拟：隔离任务超时并终止；保留原句，不阻塞游戏取文。'})[status];
      const record={fingerprint:job.fingerprint,status,detail,version:job.version,at:new Date().toLocaleTimeString('zh-CN'),sampleId:sample.id,engineName:e.name};
      if(type==='check')rememberCheck(s,e,record);
      else {record.result=status==='ready'?(sample.translations[e.target]||'此设计样句未提供该目标语言译文。'):'';u.tests[e.id]=record;}
      notice(s,'设计模拟 · '+e.name+' · '+statusNames[status]+'；只更新当前对象的记录。',status!=='ready');renderApp(app);
    },900);
  }
  function apply(s,app,ref,context) {
    const e=s.draft.localEngines[engineId(ref)],errors=validate(s,'translation',ref);
    if(!context.editable)errors.push('当前范围继承全局，请先选择专用设置。');
    if(errors.length){notice(s,errors.join(' '),true);renderApp(app);return;}
    app.dialog({title:'应用本机翻译配置？',description:'仅改变当前范围的实时翻译来源；应用与启用分开。',body:'<div class="ls-dialog-summary"><p><strong>'+esc(e.name)+'</strong> · '+esc(routeLabel(e))+'</p><p>范围：'+esc(context.scopeLabel)+'</p><p>应用后保持关闭。旧排队与在途翻译取消，原句、历史和内置词典继续。</p><p>当前检测 / 译文为设计模拟；真实系统、软件与资源兼容性仍未验证。</p></div>',confirmLabel:'应用并保持关闭',cancelLabel:'取消',onConfirm:()=>{
      context.binding.modelId='offline:'+e.id;context.binding.extraModels=Array.from(new Set(context.binding.extraModels||[])).filter(id=>id!==context.binding.modelId);context.binding.enabled=false;context.binding.historyCount=0;context.binding.outputMode='text';
      saveDraft(s);s.applied[(context.gameId||'global')+':translation']={snapshot:clone(s.draft),revision:s.revision,enabled:false,source:context.scopeLabel,savedAt:new Date().toLocaleTimeString('zh-CN')};
      const u=ensure(s),previous=u.applied[e.id],version={name:e.name,engineVersion:e.version,revision:s.revision,fingerprint:identity(s,e),scopeLabel:context.scopeLabel,scopeKey:(context.gameId||'global')+':translation'};u.applied[e.id]=Object.assign({},version,{versions:(previous?previous.versions||[previous]:[]).concat(version)});u.enabled[enabledKey(scopeKey(context),e.id)]=false;
      notice(s,'本机配置已应用；实时翻译保持关闭，请明确启用。');app.closeDialog();renderApp(app);
    }});
  }
  function toggle(s,app,e) {
    const u=ensure(s),context=contextFor(s,app),a=appliedAt(s,e,context);
    if(!context.editable||!a){notice(s,'当前范围没有可启用的已应用版本；请先在此范围重新应用。',true);renderApp(app);return;}
    const entry=s.applied[a.scopeKey],live=entry&&bindingAt(entry.snapshot,a.scopeKey);
    if(!live||live.modelId!=='offline:'+e.id){notice(s,'当前范围已由其他主翻译来源替换；请重新应用此配置后再启用。',true);renderApp(app);return;}
    const key=enabledKey(a.scopeKey,e.id),active=!!u.enabled[key]&&!!live.enabled;
    const appliedEngine=entry.snapshot.localEngines&&entry.snapshot.localEngines[e.id],gate=appliedEngine&&runtimeGate(s,appliedEngine);
    if(!active&&(!appliedEngine||a.fingerprint!==identity(s,appliedEngine)||!gate.allowed)){notice(s,gate&&!gate.allowed?gate.reason:'已应用版本的能力条件已改变，请重新检查并应用后启用。',true);renderApp(app);return;}
    if(!active&&(!readinessFor(s,appliedEngine)||readinessFor(s,appliedEngine).status!=='ready')){const health=readinessFor(s,appliedEngine);notice(s,'已应用资源'+(health?statusNames[health.status]||health.status:'需要重新检查')+'；请对该版本重新检查后启用。',true);renderApp(app);return;}
    if(!active){const errors=fieldErrors(appliedEngine);if(appliedEngine.route==='pivot'&&!appliedEngine.pivotConfirmed)errors.push('已应用版本的中转路径尚未确认。');if(errors.length){notice(s,errors.join(' '),true);renderApp(app);return;}}
    app.dialog({title:active?'停用此本机引擎？':'启用已应用的本机版本？',description:(appliedEngine&&appliedEngine.name||e.name)+' · '+a.scopeLabel,body:'<p>'+(active?'仅停止当前范围中该引擎的新任务并取消该范围的在途任务；其他游戏 / 全局范围、原文和历史保持。':'仅启用已应用 v'+a.engineVersion+'；不读取未应用草稿，不启用精读或语境查词。')+'</p><p>离线设计模拟，不访问系统翻译、语言包或 DLL。</p>',confirmLabel:active?'停用引擎':'启用已应用版本',cancelLabel:'取消',onConfirm:()=>{
      u.enabled[key]=!active;
      const setBinding=(snapshot,key)=>{const b=bindingAt(snapshot,key);if(b.modelId==='offline:'+e.id)b.enabled=!active;};
      if(entry){setBinding(entry.snapshot,a.scopeKey);entry.enabled=!active;}
      setBinding(s.draft,a.scopeKey);if(active&&u.running[e.id]&&u.running[e.id].scopeKey===a.scopeKey)cancel(s,e,'当前范围引擎已停用；旧结果丢弃。');
      notice(s,active?'本机引擎已停用；原文、历史、词典与其他引擎继续。':'已明确启用本机翻译已应用版本；实际运行能力仍未验证。');app.closeDialog();renderApp(app);
    }});
  }
  function installTools() {
    const footer=document.querySelector('.review-footer');if(!footer||document.getElementById('local-translation-design-tools'))return;
    const tools=document.createElement('details');tools.id='local-translation-design-tools';tools.className='local-design-tools';
    tools.innerHTML='<summary>设计工具 · 本机翻译模拟（待评审）</summary><p>所有检测 / 译文均为页面内模拟。无网络、下载、本机探测或 DLL 调用；不证明兼容、授权或离线能力通过。</p><label>可用性返回 <select id="local-check-outcome" data-local-tool="checkOutcome">'+outcomes.map(([id,label])=>'<option value="'+id+'">'+label+'</option>').join('')+'</select></label><label>样句返回 <select id="local-test-outcome" data-local-tool="testOutcome"><option value="ready">样句完成</option><option value="failed">执行失败</option><option value="timeout">超时</option></select></label>';
    tools.innerHTML+='<label>Apple OS / API / 策略 <select id="local-apple-scenario" data-local-tool="appleScenario"><option value="unknown">OS / API / 策略未知</option><option value="os14">macOS 14 · 不支持</option><option value="os15">macOS 15 · 基础 API</option><option value="os26">macOS 26 · headless / cancel</option><option value="os26_4_unknown">macOS 26.4 · 策略未知</option><option value="os26_4_low_latency">macOS 26.4 · lowLatency 确认（模拟）</option><option value="os26_4_high_fidelity">highFidelity / Apple Intelligence · 排除</option></select></label><label>Apple 语言对 <select id="local-apple-pair" data-local-tool="applePair"><option value="supported">支持（设计模拟）</option><option value="unknown">未知</option><option value="unsupported">不支持</option></select></label><label>Argos 运行路线 <select id="local-argos-route" data-local-tool="argosRoute"><option value="unimplemented">候选路线未实现</option><option value="pending_review">候选路线待评审</option><option value="reviewed_ready">受审路线就绪（设计模拟）</option></select></label>';
    footer.append(tools);window.LunaControls&&window.LunaControls.upgrade(tools);
  }
  document.addEventListener('click',event=>{
    const target=event.target.closest('[data-local-action]'),app=window.LunaDesign;if(!target||!app)return;
    const s=app.state.settings,u=ensure(s),e=current(s),action=target.dataset.localAction;
    if(action==='check'||action==='test'){run(s,app,e,action);return;}
    if(action==='cancel'){cancel(s,e,'用户取消；迟到结果被丢弃。');notice(s,'已取消当前本机任务；草稿与原句保留。');}
    else if(action==='next-sample'){cancel(s,e,'已换句；旧句回执被丢弃。');u.sampleIndexes[e.id]=((u.sampleIndexes[e.id]||0)+1)%samples.length;notice(s,'已换到新样句；旧结果保留归属，不覆盖新句。');}
    else if(action==='apply'||action==='toggle'){window.LunaTranslationServices?.open({origin:'settings',scope:s.scope});return;}
    else if(action==='add-current'){e.added=true;s.dirty=true;notice(s,'已添加到配置草稿；保存后进入统一选择器，可用性仍单独核验。');}
    else if(action==='new'||action==='copy'){const id=e.kind+'-'+Date.now();s.draft.localEngines[id]=action==='copy'?Object.assign(clone(e),{id,name:e.name+' 副本',installPath:'',bottle:'',packageSource:'',packageVersion:'',arch:'unknown',requiresLocalRebinding:true,version:1,pivotConfirmed:false}):config('apple_translation',id);s.draft.localEngines[id].added=true;u.selected=id;s.dirty=true;notice(s,'新配置保留在草稿中，与其他本机引擎独立；尚未检查、应用或启用。');}
    else if(action==='manage'){u.selected=engineId(contextFor(s,app).binding.modelId);s.engineFamily='offline';}
    else if(action==='view-purposes'){s.engineFamily='llm';s.tab='purposes';s.purpose='translation';}
    else if(action==='pivot'){app.dialog({title:'确认使用中转翻译？',description:routeLabel(e),body:'<p>此路径经过 English，需要两组相应语言包；质量与耗时可能变化。缺少直达语言对不会自动启用此路径。</p><p>确认仅适用于当前引擎、源目标语言与资源版本，修改后须再次确认。</p>',confirmLabel:'确认此中转路径',cancelLabel:'取消，保持未确认',onConfirm:()=>{e.pivotConfirmed=true;e.version++;s.dirty=true;notice(s,'已明确确认中转路径；请重新检查当前版本。');app.closeDialog();renderApp(app);}});return;}
    else if(action==='resource-help'){app.dialog({title:'准备本机翻译资源',description:kinds[e.kind].name,body:'<p>'+esc(kinds[e.kind].note)+'</p><p>Apple：在系统提示中授权并准备对应语言包；Argos：从可信来源自行准备运行器及语言包；J北京 / 金山：使用你已有授权的软件与词库。</p><p>本原型不会打开安装器、授权、下载或读取本机文件。准备后重新检查当前配置。</p>',confirmLabel:'我知道了',cancelLabel:'关闭',onConfirm:()=>app.closeDialog()});return;}
    renderApp(app);
  });
  function handleField(event) {
    const el=event.target.closest('[data-local-field], [data-local-tool]'),app=window.LunaDesign;if(!el||!app)return;
    const s=app.state.settings,u=ensure(s),e=current(s),key=el.dataset.localField,value=el.type==='checkbox'?el.checked:el.value,text=el.tagName==='INPUT'&&el.type==='text';
    if(event.type==='input'&&!text||event.type==='change'&&text)return;
    if(el.dataset.localTool){u[el.dataset.localTool]=value;Object.values(s.draft.localEngines).forEach(engine=>cancel(s,engine,'设计能力条件改变；旧任务回执丢弃。'));renderApp(app);return;}
    if(key==='selected'){u.selected=value;renderApp(app);return;}
    if(key==='sample'){cancel(s,e,'已换句；旧任务回执被丢弃。');u.sampleIndexes[e.id]=Number(value);renderApp(app);return;}
    if(key==='alongside'){const context=contextFor(s,app),b=context.binding,ref='offline:'+e.id;if(!context.editable||b.modelId===ref){notice(s,'主翻译来源不能重复添加为附加结果。',true);renderApp(app);return;}b.extraModels=value?Array.from(new Set((b.extraModels||[]).concat(ref))):(b.extraModels||[]).filter(id=>id!==ref);s.dirty=true;notice(s,'并列来源已保留为用途草稿；须应用用途才生效。');renderApp(app);return;}
    cancel(s,e,'配置已变化；旧任务回执被丢弃。');
    if(key==='kind'){const next=config(value,e.id);next.version=e.version+1;next.name=e.name===kinds[e.kind].name?next.name:e.name;s.draft.localEngines[e.id]=next;}
    else {e[key]=value;e.version++;if(key!=='name')e.pivotConfirmed=false;}
    if(key==='runtime'&&['windows','wine'].includes(value)&&e.kind==='apple_translation')e.runtime='native';
    s.draft.localEngines[u.selected].added=true;s.dirty=true;notice(s,'编辑保留为草稿；检测与样句按当前配置重新核验，已应用版本不变。');
    if(text){const message=document.querySelector('#settings-content .ls-footer-status span');if(message)message.textContent=s.notice;document.querySelectorAll('#settings-content [data-settings-action="save"]').forEach(button=>{button.disabled=false;});const gate=runtimeGate(s,e),ready=document.getElementById('local-readiness-status'),test=document.getElementById('local-sample-status'),freshness=document.getElementById('local-edit-freshness');if(ready)ready.innerHTML=badge(gate.allowed?checkState(s,e):gate.status);if(test)test.innerHTML=badge(testState(s,e));if(freshness)freshness.textContent='编辑已变化：当前检测 / 样句需重新核验，已应用快照保持。';document.querySelectorAll('#settings-content [data-local-action=apply]').forEach(control=>control.disabled=applyDisabled(s,app));const runTest=document.querySelector('#local-translation-config [data-local-action=test]');if(runTest)runTest.disabled=!gate.allowed||checkState(s,e)!=='ready';}
    else renderApp(app);
  }
  document.addEventListener('input',handleField);document.addEventListener('change',handleField);
  document.addEventListener('DOMContentLoaded',installTools);
  function selectionStatus(s,e) {
    if(!kinds[e.kind])return {available:false,status:'不可用',reason:'本机引擎类型不受支持。',verification:'blocked',configVersion:e.version||0};
    const fields=Object.assign(config(e.kind,e.id),e);['name','installPath','bottle','packageSource','packageVersion'].forEach(key=>fields[key]=String(fields[key]||''));
    const errors=fieldErrors(fields),gate=runtimeGate(s,e),health=readinessFor(s,e),test=ensure(s).tests[e.id];
    if(!gate.allowed)errors.push(gate.reason);
    if(!health)errors.push('当前保存版本尚未检查；资源与语言方向未知。');
    else if(health.status!=='ready')errors.push('当前资源检查：'+(statusNames[health.status]||health.status)+'。');
    if(e.route==='pivot'&&!e.pivotConfirmed)errors.push('中转语言路径尚未确认。');
    if(test&&test.fingerprint===identity(s,e)&&test.status!=='ready')errors.push('当前配置样句测试：'+(statusNames[test.status]||test.status)+'。');
    return {available:!errors.length,status:errors.length?'不可用':'检查就绪（模拟）',reason:errors.join(' '),verification:errors.length?'blocked':'passed',configVersion:e.version||0,resourceFingerprint:identity(s,e)};
  }
  window.LunaLocalTranslation={resources,ensure,isLocal,options,validate,effective,render,purposeView,apply,installTools,safeExportResources,importExample,applyDisabled,selectionStatus};
})();
