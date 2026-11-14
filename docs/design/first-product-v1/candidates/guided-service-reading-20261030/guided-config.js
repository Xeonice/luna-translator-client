/* Presentation-only form over the separately sourced service catalog. No real
   credentials, network requests, resource probes or protocol adapters run here. */
(() => {
  'use strict';
  const app=window.LunaDesign,settings=window.LunaSettings,catalog=window.LunaGuidedCatalog;
  if(!app||!settings||!catalog)throw Error('Guided presentation catalog is required');
  const clone=value=>JSON.parse(JSON.stringify(value)),esc=app.escape,views=settings.designViews;
  const services=new Map(catalog.services.map(service=>[service.id,service]));
  const protocols=new Map((window.LunaGuidedProtocolMetadata||[]).map(protocol=>[protocol.id,protocol]));
  const secretInputs=new Map(),testJobs=new Map(),ui={search:'',brand:'',service:'',purpose:'translation',testSequence:0,secretSequence:0};
  const oldStatus=settings.serviceStatus;
  const legacyPresets={traditional:'builtin.azure-translator',study:'builtin.deepseek',quick:'builtin.deepseek',local:'builtin.ollama'};
  function descriptor(resource){return services.get(resource?.guided?.serviceId);}
  function match(condition,values){if(!condition)return true;const test=rule=>rule.op==='in'?rule.value.includes(values[rule.field]):values[rule.field]===rule.value;return (condition.all||[]).every(test)&&(!condition.any||condition.any.some(test));}
  function materialBlockers(service,values){return (service.preset_material_blockers||[]).filter(blocker=>match(blocker.condition,values));}
  function presetInfoGap(service,values){const endpoint=service.fields.find(field=>field.id==='endpoint');return materialBlockers(service,values).length>0||service.kind==='http'&&values.connection_mode!=='custom'&&(service.default_missing===true||endpoint?.default_status==='not_verified_no_default'||endpoint?.default_status==='blocked_missing_default');}
  function activeFields(service,values){return service.fields.filter(field=>field.classification!=='notapplicable'&&match(field.condition,values));}
  function defaults(service,values={}){
    const result={...values};
    for(const field of service.fields)if(result[field.id]===undefined&&field.default_value!==undefined)result[field.id]=clone(field.default_value);
    for(const field of service.fields)if(field.default_by&&result[field.id]===undefined){const value=field.default_by.values[result[field.default_by.field]];if(value!==undefined)result[field.id]=clone(value);}
    return result;
  }
  function modelFor(s,id=s.selectedModel){return s.draft.models[id];}
  function current(s){return s.engineFamily==='offline'?s.draft.localEngines[window.LunaLocalTranslation.ensure(s).selected]:modelFor(s);}
  function resourceId(s){return s.engineFamily==='offline'?'offline:'+window.LunaLocalTranslation.ensure(s).selected:s.selectedModel;}
  function supports(model,purpose){const d=descriptor(model);return d?d.allowed_purposes.includes(purpose==='word'?'context_word':purpose):purpose==='translation'||model?.templatePurpose===purpose;}
  function familyFor(model,connection){return protocols.get(descriptor(model)?.protocol_id||connection?.protocol)?.family==='translation'?'cloud':'llm';}
  function hydrate(snapshot){
    for(const model of Object.values(snapshot.models||{})){
      const service=services.get(model.guided?.serviceId||legacyPresets[model.id]);if(!service)continue;
      const c=snapshot.connections[model.connectionId];
      if(!model.guided){const values=defaults(service);Object.assign(values,{model:model.modelId,endpoint:c?.endpoint,connection_mode:'custom',template:model.template,temperature:model.temperature,temperature_mode:model.temperatureMode,max_tokens:model.maxTokens,purpose:model.templatePurpose==='word'?'context_word':model.templatePurpose});model.guided={serviceId:service.id,values,secretRefs:{}};const firstSecret=service.fields.find(field=>field.type==='secret');if(firstSecret&&c?.credential)model.guided.secretRefs[firstSecret.id]=c.credential;}
      if(c){c.protocol=service.protocol_id;c.authKind=service.auth_kind;c.auth=model.guided.values.auth_mode||service.default_auth_mode;}
    }
    for(const engine of Object.values(snapshot.localEngines||{})){
      const service=services.get(engine.kind);if(service&&!engine.guided)engine.guided={serviceId:service.id,values:defaults(service,{source_language:engine.source,target_language:engine.target,installation_path:engine.installPath,runtime:engine.runtime,route_policy:engine.route}),secretRefs:{}};
    }
  }
  function init(){const s=app.state.settings;hydrate(s.draft);hydrate(s.saved);Object.values(s.applied).forEach(entry=>hydrate(entry.snapshot));const store=app.state.translationServices;if(store)for(const entry of [store.global,...Object.values(store.games||{})]){hydrate(entry.snapshot);Object.values(entry.versions||{}).forEach(hydrate);}}
  function credentialValue(resource,field){return resource.guided.secretRefs?.[field.id]||'';}
  function validation(resource,service=descriptor(resource)){
    if(!service)return ['请选择具体服务。'];const g=resource.guided,errors=[];if(presetInfoGap(service,g.values))errors.push({field:'__preset',label:'预设资料',message:materialBlockers(service,g.values).map(blocker=>blocker.reason).join('；')||'该预设资料待完善，暂不能测试或选用。'});
    for(const field of activeFields(service,g.values)){
      if(field.ui_group==='purpose')continue;
      if(field.id==='endpoint'&&g.values.connection_mode!=='custom'&&presetInfoGap(service,g.values))continue;
      if(field.required_when_visible===false||!(['required','conditional'].includes(field.classification)||field.required_when_visible===true)||field.type==='status')continue;
      const value=field.type==='secret'?credentialValue(resource,field):g.values[field.id];
      if(value===undefined||value===null||String(value).trim()==='')errors.push({field:field.id,label:field.label,message:'请'+(field.type==='action'?'完成':'填写')+field.label});
      else if(field.type==='url'){try{const parsed=new URL(value);if(!['http:','https:'].includes(parsed.protocol))throw Error('scheme');}catch{errors.push({field:field.id,label:field.label,message:field.label+'需要有效的 HTTP 或 HTTPS 地址'});}}
    }
    return errors;
  }
  function fingerprint(resource,snapshot){
    const d=descriptor(resource),g=resource.guided,fields=activeFields(d,g.values),values={},secrets={};
    for(const field of fields){if(['name','template','temperature','temperature_mode','max_tokens'].includes(field.id)||['readonly','notapplicable'].includes(field.classification)||field.ui_group==='purpose'||field.type==='status')continue;if(field.type==='secret'){if(g.secretRefs[field.id])secrets[field.id]=g.secretRefs[field.id];}else if(field.id==='key_pool')values.key_pool=!!g.values.key_pool;else if(g.values[field.id]!==undefined)values[field.id]=clone(g.values[field.id]);}
    const pool=values.key_pool?(g.poolGroups||[]).map(group=>({id:group.id,values:clone(group.values||{}),secretRefs:clone(group.secretRefs||{})})):[];
    const c=snapshot.connections?.[resource.connectionId];
    return JSON.stringify({serviceId:d.id,values,secrets,pool,poolStrategy:values.key_pool?g.poolStrategy||'roundrobin':undefined,model:resource.modelId,template:resource.template,temperatureMode:resource.temperatureMode,temperature:resource.temperatureMode==='set'?resource.temperature:undefined,maxTokens:resource.maxTokens,connection:c?{protocol:c.protocol,endpoint:c.endpoint,authKind:c.authKind,auth:c.auth,proxy:c.proxy}:null});
  }
  function settleTest(s,resource,state,message){const job=testJobs.get(resource.id);if(job){clearTimeout(job.timer);testJobs.delete(resource.id);}if(resource.guided.test?.status==='running')resource.guided.test={...resource.guided.test,status:state,message};}
  function status(s,id,snapshot=s.saved){
    const model=snapshot.models?.[id],d=descriptor(model);if(!d)return oldStatus(s,id,snapshot);
    const c=snapshot.connections[model.connectionId],errors=validation(model,d);if(s.revokedServices?.[id])errors.push({message:'此服务已删除'});if(!c||s.blockedConnections[c.id]||c.enabled===false)errors.push({message:'服务连接已停用'});
    if(model.templatePurpose!=='translation')errors.push({message:'此配置未选择翻译用途'});if(!d.allowed_purposes.includes('translation'))errors.push({message:'此服务不支持翻译'});
    if(!String(model.name||'').trim())errors.push({message:'请填写服务名称'});
    const test=(s.guidedTestRecords||[]).slice().reverse().find(record=>record.resourceId===id&&record.fingerprint===fingerprint(model,snapshot))||model.guided.test,isCurrent=test&&test.fingerprint===fingerprint(model,snapshot);if(isCurrent&&test.status==='failed')errors.push({message:test.message});
    return {available:!errors.length,status:errors.length?(isCurrent&&test.status==='failed'?'测试失败':'待配置'):isCurrent&&test.status==='passed'?'样句通过（演示）':'未验证',reason:errors.map(e=>e.message||e).join('；'),verification:isCurrent?test.status==='passed'?'passed':'failed':'unverified',configVersion:model.version||1};
  }
  function syncResource(s,resource,field,value){
    const local=!!resource.kind,g=resource.guided,beforeRequest=fingerprint(resource,s.draft);
    if(field.type==='secret'){
      const key=resource.id+':'+field.id;secretInputs.set(key,value);if(value)g.secretRefs[field.id]='design-memory:'+resource.id+':'+field.id+':'+(++ui.secretSequence);else delete g.secretRefs[field.id];
    }else g.values[field.id]=value;
    for(const dependent of descriptor(resource).fields){if(dependent.default_by?.field===field.id&&!(dependent.id==='endpoint'&&g.values.connection_mode==='custom')){const derived=dependent.default_by.values[value];if(derived!==undefined)g.values[dependent.id]=clone(derived);else delete g.values[dependent.id];}}
    if(field.id==='connection_mode'&&value==='preset'){const endpoint=descriptor(resource).fields.find(field=>field.id==='endpoint'),known=endpoint?.default_by?endpoint.default_by.values[g.values[endpoint.default_by.field]]:endpoint?.default_value;if(known!==undefined)g.values.endpoint=known;else delete g.values.endpoint;}
    if(local){const map={installation_path:'installPath',runtime:'runtime',source_language:'source',target_language:'target',route_policy:'route',language_package:'packageSource'};if(map[field.id])resource[map[field.id]]=value;}
    else {const c=s.draft.connections[resource.connectionId];if(field.id==='model')resource.modelId=String(value);if(field.id==='endpoint')c.endpoint=String(value);if(field.id==='auth_mode')c.auth=value;c.credential=Object.values(g.secretRefs)[0]||'';}
    if(field.id==='name')resource.name=String(value);
    if(!local){const c=s.draft.connections[resource.connectionId];c.endpoint=String(g.values.endpoint||'');resource.modelId=String(g.values.model||'');if(g.values.purpose)resource.templatePurpose=g.values.purpose==='context_word'?'word':g.values.purpose;const map={template:'template',temperature:'temperature',max_tokens:'maxTokens'};if(map[field.id])resource[map[field.id]]=value;if(field.id==='temperature')resource.temperatureMode='set';}
    if(field.id==='temperature_mode')resource.temperatureMode=value;
    
    if(g.test?.status==='running'&&fingerprint(resource,s.draft)!==beforeRequest)settleTest(s,resource,'stale','配置已变化，原测试已失效；需要重新测试。');resource.version=(resource.version||1)+1;s.dirty=true;s.notice='编辑尚未保存；当前服务继续使用原来的配置。';g.errors=[];
  }
  function fieldControl(field,resource){
    const g=resource.guided,mirrored={name:resource.name,model:resource.modelId,template:resource.template,temperature:resource.temperature,temperature_mode:resource.temperatureMode,max_tokens:resource.maxTokens},value=mirrored[field.id]??g.values[field.id]??'',ref=credentialValue(resource,field),key=resource.id+':'+field.id,id='guided-'+resource.id+'-'+field.id;
    const attr='id="'+esc(id)+'" data-guided-field="'+esc(field.id)+'" aria-label="'+esc(field.label)+'"';
    let control='';
    if(field.type==='status')control='<span data-guided-status="'+esc(field.id)+'">'+esc(value||'尚未检查')+'</span>';
    else if(field.type==='action')control='<button type="button" class="ls-button" data-guided-action="authorize" data-field="'+esc(field.id)+'">'+esc(field.label)+'</button><small>'+esc(value?'已关联（设计状态）':'尚未授权')+'</small>';
    else if(field.type==='select')control='<select '+attr+'>'+(!field.default_value&&value===''?'<option value="">请选择…</option>':'')+(field.choices||[]).map(choice=>'<option value="'+esc(choice.value)+'" '+(String(value)===String(choice.value)?'selected':'')+'>'+esc(choice.label)+'</option>').join('')+'</select>';
    else if(field.type==='boolean')control='<input type="checkbox" '+attr+' '+(value?'checked':'')+'>';
    else if(field.type==='textarea')control='<textarea '+attr+'>'+esc(value)+'</textarea>';
    else control='<input '+attr+' type="'+(field.type==='secret'?'password':field.type==='number'?'number':field.type==='url'?'url':'text')+'" autocomplete="off" value="'+esc(field.type==='secret'?secretInputs.get(key)||'':value)+'" placeholder="'+esc(field.type==='secret'&&ref?'已保存 · 输入可替换':field.placeholder||'')+'">';
    if(field.id==='model')control+='<button type="button" class="service-weak" data-settings-action="refresh-models">获取可用模型</button>';
    const error=(g.errors||[]).find(e=>e.field===field.id);
    return '<div class="guided-field '+(field.type==='status'?'status':'')+'" data-guided-field-row="'+esc(field.id)+'"><label for="'+esc(id)+'">'+esc(field.label)+'</label>'+control+(field.help?'<small>'+esc(field.help)+'</small>':'')+(error?'<div class="guided-field-error" role="alert">'+esc(error.message)+'</div>':'')+'</div>';
  }
  function disclosure(id,label,body,s,extraClass=''){return '<details class="guided-advanced '+extraClass+'" data-flat-disclosure="'+id+'" '+(s.flatOpen?.[id]?'open':'')+'><summary>'+label+'</summary>'+body+'</details>';}
  function advanced(resource,s,app){
    const d=descriptor(resource),g=resource.guided,fields=activeFields(d,g.values).filter(field=>field.ui_group!=='purpose'&&(field.classification==='advanced'||field.ui_group==='advanced'));let html=fields.map(field=>fieldControl(field,resource)).join('');

    if(!resource.kind){if(d.fields.some(field=>field.id==='template'&&field.classification!=='notapplicable'))html+='<button type="button" class="ls-button" data-settings-action="reset-template">恢复此用途的提示词</button>';html+='<button type="button" class="ls-button" data-guided-action="preview-resource">查看服务草稿参数…</button>';if(activeFields(d,g.values).some(field=>field.id==='key_pool'&&field.classification!=='notapplicable'))html+=poolView(resource,s);}
    else{const raw=document.createElement('div');raw.innerHTML=window.LunaLocalTranslation.render(s,app,settings.localContext(s,app));const sections=[...raw.querySelectorAll('.ls-section')];html+=disclosure('guided-local-diagnostics','本机资源检查详情',sections[1]?.outerHTML||'',s);if(resource.route==='pivot')html+='<button type="button" class="ls-button" data-local-action="pivot">确认中转翻译路径…</button>';}
    if(!resource.kind){const management=document.createElement('div');management.innerHTML=views.connectionView(s);const availability=management.querySelectorAll('.ls-section')[1];const proxy=management.querySelector('[data-settings-field="connection.proxy"]')?.closest('.ls-row');if(proxy)html+=disclosure('guided-network','网络设置（当前服务连接）',proxy.outerHTML,s);if(availability)html+=disclosure('guided-sharing','共用账号设置与停用',availability.outerHTML,s);}
    html+='<p class="ls-help">上下文、运行策略和用途覆盖在独立用途设置中编辑，不会随着本服务草稿自动更改。</p><button type="button" class="ls-button" data-guided-action="open-purpose" data-purpose="'+esc(g.values.purpose==='context_word'?'word':g.values.purpose||'translation')+'">打开相应用途设置…</button>';
    html+=disclosure('guided-identity','配置详情（排障时使用）','<dl class="guided-technical-facts"><dt>服务与接口身份</dt><dd>'+esc(d.id)+' · '+esc(d.protocol_id||d.id)+'</dd><dt>配置代次</dt><dd>'+esc(resource.version)+'</dd><dt>凭据</dt><dd>'+esc(Object.keys(g.secretRefs||{}).length?'已关联脱敏引用':'未关联')+'</dd></dl>',s);
    html+='<div class="guided-form-actions"><button type="button" class="ls-button" data-guided-action="copy-resource">复制此服务配置</button>'+(!resource.kind?'<button type="button" class="ls-button" data-settings-action="delete-model">删除服务…</button>':'')+'</div>';
    return disclosure('guided-advanced','高级设置',html,s);
  }
  function credentialFields(resource){return activeFields(descriptor(resource),resource.guided.values).filter(field=>field.type==='secret'||['app_id','app_key'].includes(field.id));}
  function poolView(resource,s){const g=resource.guided,field=descriptor(resource).fields.find(field=>field.id==='key_pool');return disclosure('guided-keys',field?.label||'多套凭据','<p class="ls-help">每套凭据单独编辑和测试；一套通过不会标记其他项。</p><label class="guided-field">调度方式<select id="guided-pool-strategy" data-guided-pool-strategy aria-label="凭据调度方式"><option value="roundrobin" '+((g.poolStrategy||'roundrobin')==='roundrobin'?'selected':'')+'>轮流使用</option><option value="weighted" '+(g.poolStrategy==='weighted'?'selected':'')+'>按错误退避</option></select></label>'+((g.poolGroups||[]).map(group=>'<div class="guided-pool-row"><span>'+esc(group.label)+'</span><small>'+esc(group.test?.message||'未测试')+'</small><button class="ls-button" data-guided-action="pool-test" data-group="'+esc(group.id)+'">单独测试</button><button class="ls-button" data-guided-action="pool-remove" data-group="'+esc(group.id)+'">移除</button></div>').join('')||'<p class="ls-help">尚未添加其他凭据。</p>')+'<button class="ls-button" data-guided-action="pool-add">添加一套凭据…</button>',s);}
  function poolDialog(resource){const fields=credentialFields(resource);ui.poolDraft={id:'group-'+Date.now(),label:'凭据 '+((resource.guided.poolGroups||[]).length+1),values:{},secretRefs:{}};app.dialog({title:'添加一套凭据',description:'以下字段只属于这一套；本页设计演示请勿填写真实密钥。',body:'<div class="guided-fields">'+fields.map(field=>'<div class="guided-field"><label>'+esc(field.label)+'</label><input data-guided-pool-field="'+esc(field.id)+'" type="'+(field.type==='secret'?'password':'text')+'" aria-label="新凭据 · '+esc(field.label)+'" autocomplete="off"></div>').join('')+'</div><p id="guided-pool-error" class="guided-field-error" role="alert"></p>',confirmLabel:'添加凭据',onConfirm:()=>{if(fields.some(field=>!ui.poolDraft[field.type==='secret'?'secretRefs':'values'][field.id])){document.getElementById('guided-pool-error').textContent='请填写这套凭据的全部必要信息。';return false;}(resource.guided.poolGroups||(resource.guided.poolGroups=[])).push(clone(ui.poolDraft));app.state.settings.dirty=true;app.closeDialog();app.render();return false;}});}
  const purposeNames={translation:'翻译',grammar:'句子精读',word:'语境查词'};
  function purposeContext(s,purpose){
    const shadow={...s,draft:clone(s.draft),purpose},gameId=s.scope==='game'?(app.state.selectedGame||app.state.session.gameId):null;
    let binding=views.formBinding(shadow,app),modelId=binding.modelId,appliedEntry=s.applied[(gameId||'global')+':'+purpose]||s.applied['global:'+purpose],selectedSnapshot;
    if(purpose==='translation'&&window.LunaTranslationServices){const selected=window.LunaTranslationServices.selection(app,gameId),store=app.state.translationServices,entry=selected.scope==='game'?store.games[gameId]:store.global;modelId=selected.mainId;selectedSnapshot=entry?.versions?.[modelId]||entry?.snapshot;if(selectedSnapshot){if(selectedSnapshot.models?.[modelId])shadow.draft.models[modelId]=clone(selectedSnapshot.models[modelId]);const connection=selectedSnapshot.connections?.[shadow.draft.models[modelId]?.connectionId];if(connection)shadow.draft.connections[connection.id]=clone(connection);}appliedEntry=selectedSnapshot?{snapshot:selectedSnapshot}:appliedEntry;}
    const model=shadow.draft.models[modelId];shadow.selectedModel=modelId;shadow.selectedConnection=model?.connectionId;shadow.engineFamily=model?familyFor(model,shadow.draft.connections[model.connectionId]):'offline';
    const scope=gameId?'当前游戏':'全局默认',effective=settings.effective(s,purpose,gameId),override=gameId&&s.draft.overrides?.[gameId]?.[purpose],inherited=!!gameId&&override?.mode!=='override';
    const appliedBinding=appliedEntry?views.formBinding({...shadow,draft:appliedEntry.snapshot},app):null;
    return {shadow,binding,model,modelId,scope,effective,inherited,appliedBinding};
  }
  function applicableField(service,id){return !!service?.fields.some(field=>field.id===id&&field.classification!=='notapplicable');}
  function policyValues(binding,service){
    if(!binding)return null;
    const keys=['language','timeout','concurrency','retries','fallback','cache'];
    if(applicableField(service,'context_count'))keys.push('historyCount','context');
    if(applicableField(service,'temperature'))keys.push('temperatureMode','temperature');
    if(applicableField(service,'template'))keys.push('templateMode','template');
    return Object.fromEntries(keys.map(key=>[key,binding[key]]));
  }
  function scopePurposeControls(root,fallback){const counts=new Map();root.querySelectorAll('[data-settings-field]').forEach(control=>{const purpose=control.closest('[data-flat-purpose]')?.dataset.flatPurpose||fallback,key=purpose+'-'+control.dataset.settingsField.replace(/[^A-Za-z0-9_-]/g,'-'),count=counts.get(key)||0;counts.set(key,count+1);control.id='guided-purpose-'+key+(count?'-'+count:'');const label=control.closest('.ls-row')?.querySelector('.ls-row-label')?.textContent||({scope:'作用范围',overrideMode:'继承方式'})[control.dataset.settingsField]||control.getAttribute('aria-label');if(label)control.setAttribute('aria-label',(purposeNames[purpose]||purpose)+' · '+label);});}
  function purposePolicy(s,purpose,includeRuntime=false){
    const context=purposeContext(s,purpose),root=document.createElement('div');
    let html='<p class="ls-help">正在编辑 '+context.scope+' · '+purposeNames[purpose]+' 用途草稿；绑定来源：'+esc(context.model?.name||context.effective.modelName||'尚未选择')+'。这与上方正在添加或编辑的服务草稿分开。</p>';
    if(includeRuntime)html+=views.scopeBar(context.shadow,app);
    const descriptorForModel=descriptor(context.model),allowsPrompt=applicableField(descriptorForModel,'template');
    if(includeRuntime&&applicableField(descriptorForModel,'context_count'))html+=views.inputRangeView(context.shadow,app,{binding:context.binding,model:context.model,effective:context.effective,inherited:context.inherited});
    if(context.model&&allowsPrompt){root.innerHTML=views.parametersView(context.shadow,app);const section=root.querySelectorAll('.ls-section')[2];if(section){section.querySelector('[data-settings-action=preview-request]')?.setAttribute('data-guided-action','preview-purpose');section.querySelector('[data-guided-action=preview-purpose]')?.setAttribute('data-purpose',purpose);section.querySelector('[data-guided-action=preview-purpose]')?.removeAttribute('data-settings-action');if(context.inherited)section.querySelectorAll('[data-settings-field^="binding."]').forEach(control=>control.disabled=true);html+=section.outerHTML;}}
    if(includeRuntime){root.innerHTML=views.runtimeView(context.shadow,app);const runtime=root.querySelectorAll('.ls-section')[2];if(runtime){if(context.inherited)runtime.querySelectorAll('[data-settings-field^="binding."]').forEach(control=>control.disabled=true);html+=runtime.outerHTML;}}
    html+='<button type="button" class="ls-button" data-guided-action="preview-purpose" data-purpose="'+purpose+'">查看用途草稿与当前已应用设置…</button>';
    if(purpose==='translation')html+='<p class="ls-help">保存只保留用途草稿；从翻译服务选择器明确选用后，后续句才引用保存的设置。当前服务、当前句与启停保持。</p><button type="button" class="ls-button" data-service-action="return">保存后选择翻译服务…</button>';
    const scoped=document.createElement('div');scoped.innerHTML=html;scopePurposeControls(scoped,purpose);html=scoped.innerHTML;
    return '<div data-flat-purpose="'+purpose+'" data-guided-purpose-policy="'+purpose+'">'+disclosure('guided-purpose-'+purpose,purposeNames[purpose]+'用途高级设置 · '+context.scope,html,s)+'</div>';
  }
  function previewPurpose(s,purpose){
    const context=purposeContext(s,purpose),service=descriptor(context.model);let request={};
    if(context.model){try{request=JSON.parse(settings.preview(context.shadow,app));}catch{request={note:'当前用途尚未形成可预览的请求。'};}
      if(protocols.get(service?.protocol_id)?.model_identity==='not_applicable')delete request.model;
      for(const [field,key] of [['template','prompt'],['temperature','temperature'],['max_tokens','max_output_tokens'],['context_count','history']])if(!applicableField(service,field))delete request[key];
    }
    return {...request,preview_scope:context.scope+' · '+purposeNames[purpose]+' 编辑草稿',bound_service:context.model?.name||context.effective.modelName,draft_policy:policyValues(context.binding,service),currently_applied_policy:policyValues(context.appliedBinding,service),effective_source:context.effective.source,network_policy:context.model?{proxy:context.shadow.draft.connections[context.model.connectionId]?.proxy}:null,note:'脱敏设计预览，不发送请求；草稿不同于当前已应用设置。'};
  }
  function learning(s,app){
    const root=document.createElement('div');root.innerHTML=views.learningPurpose(s,app,'grammar')+views.learningPurpose(s,app,'word');for(const purpose of ['grammar','word'])root.querySelector('[data-flat-purpose="'+purpose+'"]')?.insertAdjacentHTML('beforeend',purposePolicy(s,purpose));
    root.querySelectorAll('.ls-row-label').forEach(label=>{if(label.textContent==='使用配置')label.textContent='使用服务';if(label.textContent==='输出模式')label.textContent='解释样式';});
    root.querySelectorAll('[data-settings-field="binding.outputMode"]').forEach(select=>{for(const option of select.options)option.textContent=option.value==='text'?'自然语言解释':'结构与语法高亮（需支持）';});
    scopePurposeControls(root,'grammar');
    const grammar=settings.effective(s,'grammar',null),word=settings.effective(s,'word',null);
    return '<details class="guided-learning" data-flat-disclosure="guided-learning" '+(s.flatOpen?.['guided-learning']?'open':'')+'><summary>学习设置（可选）<small>句子精读：'+esc(grammar.enabled?grammar.modelName:'未启用')+' · 语境查词：'+esc(word.enabled?word.modelName:'未启用')+'</small></summary><div class="guided-form-actions"><button class="ls-button" data-guided-action="catalog" data-guided-purpose="grammar">添加精读服务…</button><button class="ls-button" data-guided-action="catalog" data-guided-purpose="context_word">添加查词服务…</button></div>'+root.innerHTML+'</details>';
  }
  function render(s,app){
    const entries=Object.values(s.saved.models||{}).map(resource=>({id:resource.id,resource,descriptor:descriptor(resource),status:resource.templatePurpose==='translation'?status(s,resource.id):{status:validation(resource).length?'待配置':'学习用途'}})).concat(Object.values(s.saved.localEngines||{}).filter(e=>e.added).map(resource=>({id:'offline:'+resource.id,resource,descriptor:descriptor(resource),status:window.LunaLocalTranslation.selectionStatus(s,resource)})));
    const drafts=Object.values(s.draft.models||{}).filter(resource=>!s.saved.models?.[resource.id]).map(resource=>({id:resource.id,resource,descriptor:descriptor(resource),status:{status:'未保存'}})).concat(Object.values(s.draft.localEngines||{}).filter(resource=>resource.added&&!s.saved.localEngines?.[resource.id]?.added).map(resource=>({id:'offline:'+resource.id,resource,descriptor:descriptor(resource),status:{status:'未保存'}})));entries.push(...drafts);
    const rows=entries.map(item=>'<button type="button" class="guided-service-row" data-guided-action="edit" data-guided-resource="'+esc(item.id)+'" '+(!(item.id.startsWith('offline:')?s.draft.localEngines[item.id.slice(8)]:s.draft.models[item.id])?'disabled':'')+'><span><strong>'+esc(item.resource.name)+'</strong><small>'+esc(item.descriptor?.label||'已有服务')+'</small></span><span class="guided-status">'+esc(item.status.status)+'</span><span>编辑</span></button>').join('');
    const resource=current(s),d=descriptor(resource);let form='';
    if(resource&&d){s.purpose=resource.kind?'translation':resource.templatePurpose||'translation';const fields=activeFields(d,resource.guided.values),required=fields.filter(field=>!['advanced','guidance'].includes(field.ui_group)&&(['required','conditional'].includes(field.classification)||field.ui_group==='necessary'));const errors=validation(resource,d),test=resource.guided.test;
      const basic=required.filter(field=>field.id!=='endpoint'||resource.guided.values.connection_mode==='custom'||!presetInfoGap(d,resource.guided.values)).map(field=>fieldControl(field,resource)).join('')+(presetInfoGap(d,resource.guided.values)?'<div class="guided-field-error" role="status">'+esc(materialBlockers(d,resource.guided.values).map(blocker=>blocker.reason).join('；')||'该预设资料待完善，暂不能测试或选用。服务地址由预设维护者补齐。')+'</div>':'');
      const testLabel=resource.kind?'检查本机条件':test?.status==='running'?'取消测试':'翻译一句试试';let localTest='';if(resource.kind){const raw=document.createElement('div');raw.innerHTML=window.LunaLocalTranslation.render(s,app,settings.localContext(s,app));const test=raw.querySelector('[data-local-action=test]');localTest='<button type="button" class="ls-button" data-local-action="test" '+(test?.disabled?'disabled':'')+'>翻译一句试试</button>';}
      form='<section class="guided-editor" data-guided-section="editor" data-guided-service="'+esc(d.id)+'" data-guided-resource="'+esc(resourceId(s))+'"><h3>'+esc(resource.name)+'</h3><div class="guided-editor-note">'+esc(d.label)+' · '+(d.kind==='offline'?'仅在本机使用':'需要时才发送测试样句')+'</div><div class="guided-editor-summary">'+'用于'+esc(({translation:'翻译',grammar:'句子精读',context_word:'语境查词'})[resource.guided.values.purpose]||'翻译')+(d.interface_choice_required?' · '+esc(d.interface_label||d.label):'')+'<button type="button" class="service-weak" data-guided-action="catalog">更换服务…</button></div><div class="guided-fields">'+basic+'</div><div class="guided-form-actions"><button type="button" class="ls-button" data-guided-action="'+(resource.kind?'check-local':test?.status==='running'?'cancel-test':'test')+'" '+(!resource.kind&&errors.length&&test?.status!=='running'?'disabled':'')+'>'+testLabel+'</button>'+localTest+'<small>保存不切换当前服务；测试结果仅属于这项配置。</small></div><div class="guided-test-result" role="status">'+esc(resource.kind?window.LunaLocalTranslation.selectionStatus(s,resource).reason:(test?(test.fingerprint&&test.fingerprint!==fingerprint(resource,s.draft)?'设置已变化，需要重新测试。':test.message):errors.length?'还需填写：'+errors.map(e=>e.label).join('、'):'尚未测试；可以先保存。'))+'</div>'+advanced(resource,s,app)+'</section>';
    }
    return '<header class="guided-heading"><h2>翻译服务</h2><p>选择你要使用的服务，再填写必要的信息。</p></header><section class="guided-saved" data-guided-section="saved"><div class="guided-section-head"><h3>已添加的服务</h3><button class="ls-button" data-guided-action="catalog">添加服务…</button></div>'+(rows||'<p class="flat-empty">还没有添加服务。从熟悉的服务名称开始。</p>')+'</section>'+form+purposePolicy(s,'translation',true)+learning(s,app);
  }
  function catalogDialog(brandId){
    const brands=Array.isArray(catalog.brands)?catalog.brands:Object.values(catalog.brands),brand=brands.find(item=>item.id===brandId);
    if(brand){ui.brand=brandId;const options=catalog.services.filter(service=>service.brand_id===brandId&&service.allowed_purposes.includes(ui.purpose));app.dialog({title:'选择'+brand.label+'的连接方式',description:'只按你选定的接口配置；发生错误时不会自动换成其他接口。',body:'<div class="guided-interfaces">'+options.map((service,index)=>'<button type="button" data-guided-action="choose-service" data-guided-service="'+esc(service.id)+'"><strong>'+esc(service.interface_label||service.label)+'</strong><small>'+esc(service.interface_description||service.label)+(brand.default_service_id===service.id?' · 建议选项，仍需你确认':'')+'</small></button>').join('')+'</div>',confirmLabel:'返回服务列表',cancelLabel:'取消',onConfirm:()=>{catalogDialog();return false;}});return;}
    ui.search='';app.dialog({title:'添加服务',description:'选择你已经使用的服务，或本机已有的翻译引擎。',body:'<label class="guided-catalog-purpose">用来做什么<select id="guided-catalog-purpose" data-guided-purpose-control aria-label="服务用途"><option value="translation" '+(ui.purpose==='translation'?'selected':'')+'>翻译</option><option value="grammar" '+(ui.purpose==='grammar'?'selected':'')+'>句子精读</option><option value="context_word" '+(ui.purpose==='context_word'?'selected':'')+'>语境查词</option></select></label><input class="guided-catalog-search" data-guided-search aria-label="搜索服务" placeholder="搜索服务名称"><div class="guided-brands">'+brands.map(item=>'<button type="button" class="guided-brand" data-guided-action="brand" data-guided-brand="'+esc(item.id)+'" '+(!catalog.services.some(service=>service.brand_id===item.id&&service.allowed_purposes.includes(ui.purpose))?'disabled':'')+'><strong>'+esc(item.label)+'</strong><small>'+esc(item.description||'')+'</small></button>').join('')+'</div>',confirmLabel:'关闭',cancelLabel:'取消',onConfirm:()=>app.closeDialog()});
  }
  function selectService(serviceId){
    const d=services.get(serviceId);if(!d)throw Error('Unknown catalog service: '+serviceId);if(!d.allowed_purposes.includes(ui.purpose))throw Error('Service does not support selected purpose');const s=app.state.settings,serial=Date.now()+'-'+(++ui.testSequence);
    if(d.kind==='offline'){
      const source=s.draft.localEngines[d.id]||settings.initialState().draft.localEngines[d.id],id=d.id+'-'+serial;
      const resource={...clone(source),id,added:true,name:d.label,guided:{serviceId:d.id,values:{...defaults(d),purpose:ui.purpose},secretRefs:{}}};resource.guided.values=defaults(d,{...resource.guided.values,source_language:resource.guided.values.source_language||resource.source,target_language:resource.guided.values.target_language||resource.target,runtime:resource.guided.values.runtime||resource.runtime});s.draft.localEngines[id]=resource;s.engineFamily='offline';window.LunaLocalTranslation.ensure(s).selected=id;
    }else{
      const id='service-'+serial,cid='connection-'+serial,proto=protocols.get(d.protocol_id),values={...defaults(d),purpose:ui.purpose},templates=settings.initialState().draft.models;
      s.draft.connections[cid]={id:cid,name:d.label,protocol:d.protocol_id,location:['ollama','lmstudio'].includes(d.brand_id)?'local':'cloud',endpoint:values.endpoint||'',auth:values.auth_mode||d.default_auth_mode,authKind:d.auth_kind,credential:'',proxy:'inherit',enabled:true,version:1};
      s.draft.models[id]={id,name:d.label,connectionId:cid,modelId:values.model||'',capability:proto?.family==='translation'?'translation':'text',template:templates[ui.purpose==='grammar'?'study':ui.purpose==='context_word'?'quick':'traditional'].template,templatePurpose:ui.purpose==='context_word'?'word':ui.purpose,outputMode:'text',temperatureMode:'inherit',temperature:'',maxTokens:'',version:1,guided:{serviceId:d.id,values,secretRefs:{}}};s.selectedModel=id;s.selectedConnection=cid;s.engineFamily=familyFor(s.draft.models[id],s.draft.connections[cid]);
    }
    s.purpose='translation';s.dirty=true;s.notice='已添加配置草稿，填写后保存；当前服务保持不变。';app.closeDialog();app.render();document.querySelector('[data-guided-section=editor]')?.scrollIntoView({block:'start'});
  }
  function edit(id){const s=app.state.settings;if(id.startsWith('offline:')){s.engineFamily='offline';s.purpose='translation';window.LunaLocalTranslation.ensure(s).selected=id.slice(8);}else {const model=s.draft.models[id];if(!model){s.notice='此配置已经移除。';app.render();return;}s.selectedModel=id;s.selectedConnection=model.connectionId;s.engineFamily=familyFor(model,s.draft.connections[model.connectionId]);s.purpose=model.templatePurpose||'translation';}app.render();document.querySelector('[data-guided-section=editor]')?.scrollIntoView({block:'start'});}
  function save(s){const resource=current(s);if(resource?.guided)resource.guided.errors=validation(resource);views.save(s);Object.values(s.saved.models||{}).forEach(model=>{if(model.guided){delete model.guided.test;delete model.guided.errors;}});secretInputs.clear();s.notice=resource?.guided?.errors.length?'草稿已保存；还需完成必要信息。':'配置已保存；当前服务与自动翻译保持不变。';app.render();}
  function test(s){
    const resource=current(s),errors=validation(resource);resource.guided.errors=errors;if(errors.length){app.render();return;}
    settleTest(s,resource,'cancelled','已取消被后续测试替代的任务。');
    const before=fingerprint(resource,s.draft),id=resource.id,outcome=s.testOutcome,job={fingerprint:before};
    resource.guided.test={status:'running',message:'正在处理本页设计样句…',fingerprint:before};testJobs.set(id,job);app.render();
    job.timer=setTimeout(()=>{const target=s.draft.models[id];if(testJobs.get(id)!==job)return;if(!target){settleTest(s,resource,'cancelled','配置已移除，测试取消。');return;}if(fingerprint(target,s.draft)!==before){settleTest(s,target,'stale','配置已变化，原测试已失效；需要重新测试。');app.render();return;}
      testJobs.delete(id);target.guided.test={status:outcome==='pass'?'passed':'failed',message:outcome==='pass'?'设计样句已返回；真实服务尚未验证。':outcome==='auth'?'认证失败，请检查必要凭据。':outcome==='timeout'?'测试超时，配置已保留。':'输出不符合当前用途。',fingerprint:before};(s.guidedTestRecords||(s.guidedTestRecords=[])).push({resourceId:id,...clone(target.guided.test),at:new Date().toISOString()});app.render();
    },500);
  }
  document.addEventListener('click',event=>{
    const resourceCommand=event.target.closest('[data-settings-action]');if(resourceCommand?.closest('[data-guided-section=editor]')){const resource=current(app.state.settings);app.state.settings.purpose=resource.kind?'translation':resource.templatePurpose||'translation';}
    const saveControl=event.target.closest('#settings-content [data-settings-action=save]');if(saveControl&&app.state.settings.category==='translation'){event.preventDefault();event.stopImmediatePropagation();save(app.state.settings);return;}
    const button=event.target.closest('[data-guided-action]');if(!button)return;event.preventDefault();event.stopImmediatePropagation();const s=app.state.settings,action=button.dataset.guidedAction;
    if(action==='catalog'){ui.purpose=button.dataset.guidedPurpose||'translation';catalogDialog();}
    else if(action==='brand'){const id=button.dataset.guidedBrand,options=catalog.services.filter(service=>service.brand_id===id&&service.allowed_purposes.includes(ui.purpose));if(options.length===1)selectService(options[0].id);else catalogDialog(id);}
    else if(action==='choose-service')selectService(button.dataset.guidedService);
    else if(action==='edit')edit(button.dataset.guidedResource);
    else if(action==='copy-resource'){const original=current(s),g=clone(original.guided),serial=Date.now()+'-'+(++ui.testSequence);g.secretRefs={};g.poolGroups=[];delete g.test;delete g.errors;if(original.kind){const hidden=document.createElement('button');hidden.dataset.localAction='copy';hidden.hidden=true;document.body.append(hidden);hidden.click();hidden.remove();const copy=current(s);copy.guided=g;copy.guided.values.installation_path=copy.installPath;delete copy.guided.values.language_package;delete copy.guided.values.bottle;copy.guided.errors=[];}else{const originalConnection=s.draft.connections[original.connectionId],cid='connection-copy-'+serial,id='service-copy-'+serial;s.draft.connections[cid]={...clone(originalConnection),id:cid,name:originalConnection.name+' 副本',credential:'',version:1};s.draft.models[id]={...clone(original),id,name:original.name+' 副本',connectionId:cid,version:1,guided:g};s.selectedModel=id;s.selectedConnection=cid;}s.dirty=true;s.notice='已复制为独立草稿；凭据需重新填写，本机资源需重新绑定。';app.render();}
    else if(action==='open-purpose'){const purpose=button.dataset.purpose;s.flatOpen||(s.flatOpen={});s.flatOpen['guided-purpose-'+purpose]=true;if(purpose!=='translation')s.flatOpen['guided-learning']=true;app.render();document.querySelector('[data-guided-purpose-policy="'+purpose+'"]')?.scrollIntoView({block:'start'});}
    else if(action==='preview-purpose'){const data=previewPurpose(s,button.dataset.purpose);app.dialog({title:'用途请求与策略预览 · 编辑草稿',description:'只引用所标明用途与范围，不借用正在新增的服务。',body:'<pre class="ls-request">'+esc(JSON.stringify(data,null,2))+'</pre>',confirmLabel:'完成',cancelLabel:'关闭'});}
    else if(action==='preview-resource'){const resource=current(s),c=s.draft.connections[resource.connectionId];app.dialog({title:'服务草稿参数预览',description:'仅查看此资源草稿；用途上下文和运行策略在独立用途设置中预览。',body:'<pre class="ls-request">'+esc(JSON.stringify({service:descriptor(resource).label,endpoint:c.endpoint,model:descriptor(resource).model_identity==='not_applicable'?undefined:resource.modelId,network:{proxy:c.proxy},temperatureMode:resource.temperatureMode,temperature:resource.temperatureMode==='set'?resource.temperature:undefined,maxOutputTokens:resource.maxTokens,template:resource.template},null,2))+'</pre>',confirmLabel:'完成',cancelLabel:'关闭'});}
    else if(action==='test')test(s);
    else if(action==='cancel-test'){settleTest(s,current(s),'cancelled','测试已取消；配置保留。');app.render();}
    else if(action==='check-local'){const hidden=document.createElement('button');hidden.dataset.localAction='check';hidden.hidden=true;document.body.append(hidden);hidden.click();hidden.remove();}
    else if(action==='pool-add')poolDialog(current(s));
    else if(action==='pool-remove'){const g=current(s).guided;g.poolGroups=g.poolGroups.filter(group=>group.id!==button.dataset.group);s.dirty=true;app.render();}
    else if(action==='pool-test'){const group=current(s).guided.poolGroups.find(group=>group.id===button.dataset.group);group.test={message:s.testOutcome==='pass'?'此套样例通过（演示）':'此套样例失败（演示）'};app.render();}
    else if(action==='authorize')app.dialog({title:'连接授权账号',description:'此页面只演示配置界面，不会发起真实登录。',body:'<p>真实客户端会使用你明确选择的授权方式与权限范围。当前尚未授权，不会把此服务标成可选。</p>',confirmLabel:'知道了',cancelLabel:'关闭'});
  },true);
  ['input','change'].forEach(type=>document.addEventListener(type,event=>{
    const poolField=event.target.closest('[data-guided-pool-field]');if(poolField&&type==='input'){const field=credentialFields(current(app.state.settings)).find(field=>field.id===poolField.dataset.guidedPoolField);if(field.type==='secret'){secretInputs.set(ui.poolDraft.id+':'+field.id,poolField.value);if(poolField.value)ui.poolDraft.secretRefs[field.id]='design-memory:'+ui.poolDraft.id+':'+field.id+':'+(++ui.secretSequence);else delete ui.poolDraft.secretRefs[field.id];}else ui.poolDraft.values[field.id]=poolField.value;return;}
    const strategy=event.target.closest('[data-guided-pool-strategy]');if(strategy&&type==='change'){current(app.state.settings).guided.poolStrategy=strategy.value;app.state.settings.dirty=true;app.render();return;}
    const purpose=event.target.closest('[data-guided-purpose-control]');if(purpose){if(type==='change'){ui.purpose=purpose.value;catalogDialog();}return;}
    const search=event.target.closest('[data-guided-search]');if(search){document.querySelectorAll('[data-guided-brand]').forEach(button=>button.hidden=!button.textContent.toLowerCase().includes(search.value.toLowerCase()));return;}
    const name=event.target.closest('[data-guided-name]');const input=event.target.closest('[data-guided-field]');if(!input&&!name)return;
    const text=name||input.tagName==='TEXTAREA'||input.tagName==='INPUT'&&input.type!=='checkbox';if(type==='input'&&!text||type==='change'&&text)return;
    event.stopImmediatePropagation();const s=app.state.settings,resource=current(s);if(name){resource.name=name.value;s.dirty=true;}else{const field=descriptor(resource).fields.find(field=>field.id===input.dataset.guidedField);syncResource(s,resource,field,input.type==='checkbox'?input.checked:input.value);}
    document.querySelectorAll('#settings-content [data-settings-action=save]').forEach(control=>control.disabled=!s.dirty);
    if(text){const message=document.querySelector('.ls-footer-status span');if(message)message.textContent='有未保存的编辑；当前服务保持不变。';const testControl=document.querySelector('[data-guided-action=test],[data-guided-action=cancel-test]');if(testControl&&resource.guided.test?.status!=='running'){testControl.dataset.guidedAction='test';testControl.textContent='翻译一句试试';}const errors=validation(resource);if(testControl)testControl.disabled=errors.length>0;const result=document.querySelector('.guided-test-result');if(result&&!resource.kind)result.textContent=errors.length?'还需填写：'+errors.map(error=>error.label).join('、'):resource.guided.test?(resource.guided.test.status==='running'?resource.guided.test.message:resource.guided.test.fingerprint&&resource.guided.test.fingerprint!==fingerprint(resource,s.draft)?'设置已变化，需要重新测试。':resource.guided.test.message):'尚未测试；可以先保存。';input?.closest('.guided-field')?.querySelector('.guided-field-error')?.remove();}
    else app.render();
  },true));
  function sanitizeExport(draft){Object.values(draft.connections||{}).forEach(c=>{c.credential='';});Object.values(draft.models||{}).forEach(m=>{if(m.guided){m.guided.secretRefs={};(m.guided.poolGroups||[]).forEach(group=>{group.secretRefs={};delete group.test;});delete m.guided.test;delete m.guided.errors;}});return draft;}
  function validateModel(s,purpose,id){const resource=s.draft.models[id||views.formBinding(s,app)?.modelId];if(!descriptor(resource))return [];const errors=validation(resource).map(error=>error.message||String(error));if(!supports(resource,purpose))errors.push('所选服务不支持此用途。');return errors;}
  function requiresModel(model,connection){return protocols.get(descriptor(model)?.protocol_id||connection?.protocol)?.model_identity==='required';}
  settings.serviceStatus=status;window.LunaGuidedConfig={validateModel,requiresModel,sanitizeExport,render,services,catalog,match,activeFields,validation,familyFor,supports,selectService,edit,resourceId};init();
})();
