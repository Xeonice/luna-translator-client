/* User-visible library flows for the offline design. No process or filesystem access. */
(function () {
  'use strict';
  const store = {catalogue:[],records:new Map(),owned:new Set(),bindings:new Map(),add:null,running:null,app:null,sequence:0};
  const clone = value => JSON.parse(JSON.stringify(value));
  const esc = value => String(value == null ? '' : value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const icon = name => window.LunaIcons ? window.LunaIcons.svg(name) : '';
  const norm = value => String(value||'').trim().replace(/[\\/]+/g,'\\').replace(/\\$/,'').toLowerCase();
  const basename = value => String(value||'').split(/[\\/]+/).pop()||'';
  const dirname = value => String(value||'').replace(/[\\/]+[^\\/]*$/,'');
  const environmentName = value => ({crossover:'CrossOver',wine:'Wine',native_mac:'原生 macOS'}[value]||'待核对');
  function identity(game) {return [game.environment||'',norm(game.bottle),norm(game.targetPath||game.path)].join('|');}
  function fixtureFor(game) {return store.catalogue.find(item=>identity(item)===identity(game)&&item.arch===game.arch)||null;}
  function sanitize(raw,id) {
    const clean=Object.assign({},clone(raw),{id:id||raw.id,environment:raw.environment||'crossover',title:String(raw.title||'').trim(),bottle:String(raw.bottle||'').trim(),launchPath:String(raw.launchPath||'').trim(),targetPath:String(raw.targetPath||raw.path||'').trim(),args:String(raw.args||''),cwd:String(raw.cwd||''),arch:raw.arch||'unknown'});
    clean.path=clean.targetPath;clean.exe=basename(clean.targetPath||clean.launchPath)||raw.exe||'待核对';
    const fixture=fixtureFor(clean);
    if(fixture){const metadata={title:clean.title,environment:clean.environment,bottle:clean.bottle,launchPath:clean.launchPath,targetPath:clean.targetPath,path:clean.targetPath,args:clean.args,cwd:clean.cwd,arch:clean.arch,exe:clean.exe,id:clean.id};Object.assign(clean,clone(fixture),metadata,{fixtureSampleId:fixture.id});}
    else{clean.streams=[];clean.lines=[];clean.scene=null;clean.cover=null;clean.hook='待验证';clean.language='ja';clean.subtitle='自定义记录 · 接入待验证';clean.fixtureSampleId=null;}
    return clean;
  }
  function initialise(app,{restored=false}={}) {
    store.app=app;store.records=new Map();store.owned=new Set();store.bindings=new Map();store.add=null;store.running=null;
    store.catalogue=(window.LunaDesignData&&window.LunaDesignData.games||[]).map(game=>{
      const path=String(game.targetPath||game.path||'').replace(/\\+/g,'\\');
      return Object.assign(clone(game),{environment:'crossover',launchPath:path,targetPath:path,path,args:'',cwd:dirname(path),fixtureSampleId:game.id});
    });
    if(restored)store.catalogue.forEach(game=>{store.records.set(game.id,clone(game));store.owned.add(game.id);});
    return libraryGames();
  }
  function allGames(){const result=new Map(store.catalogue.map(game=>[game.id,game]));store.records.forEach((game,id)=>result.set(id,game));return Array.from(result.values());}
  function libraryGames(){return Array.from(store.owned).map(id=>store.records.get(id)).filter(Boolean);}
  function byId(id){return store.records.get(id)||store.catalogue.find(game=>game.id===id)||null;}
  function isRunning(game){return !!game&&!!fixtureFor(game);}
  function windowChoices(){
    return store.catalogue.map((game,index)=>({id:'sample-window-'+game.id,title:game.title,hostWindowId:'design-window-'+(101+index),hostPid:41000+index*13,windowsPid:6200+index*8,processStartedAt:'design-start-'+game.id,bottle:game.bottle,targetPath:game.targetPath,launchPath:game.launchPath,cwd:game.cwd,args:'',recordId:game.id,environment:game.environment,arch:game.arch,exe:game.exe,scene:game.scene})).concat([{id:'luna-self',title:'Luna 主窗口',hostWindowId:'design-window-luna',hostPid:40000,windowsPid:null,bottle:'',targetPath:'',recordId:null,environment:'native_mac',arch:'arm64',exe:'Luna.app',scene:null,self:true}]);
  }
  function getBinding(id,target){const binding=store.bindings.get(id);if(!binding)return null;if(target&&(identity(binding)!==identity(target)||binding.arch!==target.arch))return null;return clone(binding);}
  function bindingFrom(choice,id){return {title:choice.title,hostWindowId:choice.hostWindowId,hostPid:choice.hostPid,windowsPid:choice.windowsPid,processStartedAt:choice.processStartedAt,bottle:choice.bottle,targetPath:choice.targetPath,recordId:id,environment:choice.environment,arch:choice.arch};}
  function unbind(app,id){store.bindings.delete(id);app.render();app.notify('已解除窗口关联；Hook 会话和当前原句保持不变。');}
  function onConnected(app,id){const game=byId(id),choice=game&&fixtureFor(game)&&windowChoices().find(item=>identity(item)===identity(game));if(choice)store.bindings.set(id,bindingFrom(choice,id));app.render();}
  function onGameExit(app,id){store.bindings.delete(id);}
  function field(label,name,value,{type='text',placeholder='',note='',options=null,required=false}={}){
    const control=options?'<select data-gl-field="'+name+'" aria-label="'+esc(label)+'">'+options.map(([v,t])=>'<option value="'+esc(v)+'" '+(String(v)===String(value)?'selected':'')+'>'+esc(t)+'</option>').join('')+'</select>':'<input type="'+type+'" data-gl-field="'+name+'" aria-label="'+esc(label)+'" value="'+esc(value)+'" placeholder="'+esc(placeholder)+'">';
    return '<label class="gl-field"><span>'+label+(required?'<span class="gl-required"> · 必填</span>':'')+'</span><div>'+control+(note?'<small>'+note+'</small>':'')+'</div></label>';
  }
  function resourceForm(record){
    return '<div class="gl-metadata">'+field('游戏名称','title',record.title,{required:true})+field('运行环境','environment',record.environment,{options:[['crossover','CrossOver'],['wine','Wine'],['native_mac','原生 macOS · 仅建库']]})+field('容器名称','bottle',record.bottle,{required:record.environment!=='native_mac',placeholder:'与目标运行容器一致'})+field('启动路径','launchPath',record.launchPath,{required:true,placeholder:'C:\\Games\\Example\\Game.exe',note:'从文件添加仅建立记录。启动路径可与实际取文进程路径不同。'})+field('目标进程路径','targetPath',record.targetPath,{required:true,placeholder:'核对实际游戏进程的完整路径',note:'匹配环境、容器与完整目标路径；不只比较 exe 文件名。'})+field('目标架构','arch',record.arch,{options:[['unknown','待核对'],['x86','32 位 · x86'],['x64','64 位 · x64']]})+field('启动参数','args',record.args,{placeholder:'可留空'})+field('工作目录','cwd',record.cwd,{placeholder:'可留空，使用程序所在目录'})+'</div>';
  }
  function collectCurrent(){const ui=store.add;if(!ui||!ui.items[ui.selected])return;const record=ui.items[ui.selected];document.querySelectorAll('#app-dialog [data-gl-field]').forEach(input=>{const key=input.dataset.glField;if(Object.prototype.hasOwnProperty.call(record,key))record[key]=input.value;});}
  function addError(message){const node=document.querySelector('#app-dialog .gl-errors');if(node){node.textContent=message;node.hidden=!message;}return false;}
  function validation(record){
    if(!record.title.trim())return '请填写游戏名称。';
    if(record.environment!=='native_mac'&&!record.bottle.trim())return '请填写 CrossOver / Wine 的容器名称。';
    if(!record.launchPath.trim()||!record.targetPath.trim())return '请核对启动路径与目标进程完整路径。仅文件名不足以建立运行身份。';
    const pathPattern=record.environment==='native_mac'?/^\//:/^[A-Za-z]:[\\/]/;
    if(!pathPattern.test(record.launchPath)||!pathPattern.test(record.targetPath))return record.environment==='native_mac'?'原生 macOS 路径应为绝对路径。':'Windows 路径应为盘符开头的完整路径，例如 C:\\Games\\Example\\Game.exe。';
    return '';
  }
  function newRecord(name){return {id:null,title:String(name||'').replace(/\.(exe|lnk|app)$/i,''),environment:'crossover',bottle:'',launchPath:'',targetPath:'',args:'',cwd:'',arch:'unknown',fileName:name||'',filenameOnly:true};}
  function storeRecord(raw,editingId){
    const existing=allGames().find(item=>identity(item)===identity(raw));
    const id=editingId||existing&&existing.id||'game-'+Date.now()+'-'+(++store.sequence);
    const record=sanitize(raw,id),previous=store.records.get(id);store.records.set(id,record);store.owned.add(id);
    if(previous&&(identity(previous)!==identity(record)||previous.arch!==record.arch))store.bindings.delete(id);
    return record;
  }
  function renderAdd(){
    const ui=store.add,app=store.app,record=ui.items[ui.selected],editing=!!ui.editingId;
    let body='<div class="gl-flow gl-add"><p class="gl-design-note">'+icon('circle')+'<span>设计回放：文件选择只读取文件名；列表与运行信息不是本机扫描结果。</span></p>';
    if(!editing){body+='<div class="gl-drop-zone" data-gl-drop tabindex="0" role="button" aria-label="选择或拖入游戏程序"><span class="gl-drop-icon">'+icon('window')+'</span><strong>选择或拖入游戏程序</strong><span>支持多个 .exe / .lnk；添加后核对路径与容器</span><button type="button" data-gl-action="browse-files">选择文件…</button><input id="gl-game-files" type="file" multiple accept=".exe,.lnk,.app" hidden></div><div class="gl-chooser-row"><span>也可以体验已知游戏的设计数据</span><button type="button" data-gl-action="show-fixtures">选择示例程序…</button></div>';
      if(ui.showFixtures)body+='<div class="gl-fixture-list" aria-label="示例程序">'+store.catalogue.map(game=>'<button type="button" data-gl-action="add-fixture" data-gl-id="'+game.id+'"><span>'+icon('window')+'</span><div><strong>'+esc(game.title)+'</strong><small>'+esc(game.bottle)+' · '+esc(game.exe)+'</small></div><span class="gl-small-label">添加示例</span></button>').join('')+'</div>';
    }
    if(ui.items.length){
      body+='<div class="gl-preview-heading"><h3>'+ (editing?'核对游戏资料':'待添加 · '+ui.items.length+' 项')+'</h3><span>保存只建立游戏记录</span></div>';
      if(ui.items.length>1)body+='<div class="gl-import-items" aria-label="批量添加项目">'+ui.items.map((item,i)=>'<button type="button" class="'+(i===ui.selected?'selected':'')+'" data-gl-action="select-item" data-gl-index="'+i+'"><span>'+icon('window')+'</span><span>'+esc(item.title||item.fileName||'未命名程序')+'</span><small>'+esc(item.bottle||'待核对')+'</small></button>').join('')+'</div>';
      body+=resourceForm(record)+'<p class="gl-identity-note">'+(fixtureFor(record)?icon('check')+'<span>完整身份匹配既有游戏设计素材，可继续体验该游戏的样例流程。</span>':icon('alert')+'<span>此程序没有已匹配的采集素材。保存后可建库；不会显示其他游戏的原文或假装取文成功。</span>')+'</p>';
    }
    body+='<div class="gl-errors" role="alert" hidden></div></div>';
    app.dialog({title:editing?'编辑游戏资料':'添加游戏',description:editing?'修改资料不会启动程序。运行身份变化时清除旧窗口关联与样例内容。':'从文件添加只建立游戏记录。需要连接时，再选择已运行的目标。',body,confirmLabel:editing?'保存资料':ui.connect?'添加并连接…':'添加到游戏库',cancelLabel:'取消',onConfirm:()=>{
      collectCurrent();if(!ui.items.length)return addError('先选择游戏文件，或选择一个明确标记的示例程序。');
      for(let i=0;i<ui.items.length;i++){const duplicate=ui.editingId&&libraryGames().find(game=>game.id!==ui.editingId&&identity(game)===identity(ui.items[i]));const error=duplicate?'此目标已属于“'+duplicate.title+'”，请编辑原记录或核对不同容器/路径，避免建立重复游戏。':validation(ui.items[i]);if(error){ui.selected=i;renderAdd();return addError('第 '+(i+1)+' 项：'+error);}}
      const saved=ui.items.map(item=>storeRecord(item,ui.editingId)),uniqueCount=new Set(saved.map(game=>game.id)).size;store.add=null;app.state.selectedGame=saved[0].id;app.closeDialog();app.render();app.notify(editing?'游戏资料已保存；未启动程序。':'已保存 '+uniqueCount+' 款游戏记录；重复目标已复用，文件与运行进程保持原状。');if(ui.connect)app.connectGame(saved[0].id);return false;
    }});
    const confirm=document.querySelector('#dialog-confirm');if(confirm)confirm.disabled=!ui.items.length;
  }
  function openAdd(app,{connect=false}={}){store.app=app;store.add={connect,items:[],selected:0,showFixtures:false,editingId:null};renderAdd();}
  function openEdit(app,id){const record=byId(id);if(!record)return;store.app=app;store.add={connect:false,items:[clone(record)],selected:0,showFixtures:false,editingId:id};renderAdd();}
  function remove(app,id){const record=byId(id);if(!record)return;app.dialog({title:'从游戏库移除此游戏？',description:record.title,body:'<div class="gl-flow"><p>只移除游戏库条目，保留游戏文件、阅读历史和已保存的配置。当前 Hook 会话不会因此切换或断开。</p></div>',confirmLabel:'移出游戏库',cancelLabel:'取消',onConfirm:()=>{store.owned.delete(id);if(app.state.selectedGame===id)app.state.selectedGame=libraryGames()[0]&&libraryGames()[0].id||null;app.render();}});}
  function sameRuntimeTarget(choice,session){
    if(!choice||!session||session.status!=='connected')return false;
    const current=session.runtimeTarget||windowChoices().find(item=>!item.self&&identity(item)===identity(session.gameSnapshot||{})&&item.arch===session.gameSnapshot?.arch);
    return !!current&&identity(current)===identity(choice)&&current.arch===choice.arch&&current.windowsPid===choice.windowsPid&&current.processStartedAt===choice.processStartedAt;
  }
  function runningForm(){
    const ui=store.running,app=store.app,choice=windowChoices().find(item=>item.id===ui.selectedId),game=ui.expectedGame;
    const active=app.state.session?.status==='connected',reuse=choice&&sameRuntimeTarget(choice,app.state.session),switching=choice&&!choice.self&&active&&!reuse;
    let body='<div class="gl-flow gl-running"><p class="gl-design-note">'+icon('circle')+'<span>离线设计回放 · 目标和接入进度为设计样例，不访问本机进程。</span></p>';
    if(game)body+='<div class="gl-binding-context"><span>'+icon('pin')+'</span><div><strong>'+esc(game.title)+'</strong><small>'+(ui.activeSession?'当前活动会话 · '+esc(game.exe)+' · '+esc(game.arch):'游戏库目标 · '+esc(game.exe)+' · '+esc(game.arch))+'<br>选择后先核对；确认“关联并接入”会同步启动文本接入。</small></div>'+(getBinding(game.id,game)?'<button type="button" data-gl-action="clear-binding">解除现有关联</button>':'')+'</div>';
    if(ui.launch&&game)body+='<p class="gl-identity-note">'+icon('play')+'<span>运行并连接：按保存的启动路径、参数和工作目录启动后，核对实际运行窗口。本离线设计稿不执行启动命令。</span></p>';
    body+='<div class="gl-running-toolbar"><div class="gl-segment" role="tablist" aria-label="目标来源"><button type="button" role="tab" aria-selected="'+(ui.list==='windows')+'" class="'+(ui.list==='windows'?'selected':'')+'" data-gl-action="running-tab" data-gl-value="windows">运行窗口</button><button type="button" role="tab" aria-selected="'+(ui.list==='processes')+'" class="'+(ui.list==='processes'?'selected':'')+'" data-gl-action="running-tab" data-gl-value="processes">进程列表</button></div><button type="button" data-gl-action="pick-window">'+icon('scan')+' 点击选择窗口…</button></div>';
    if(ui.picking)body+='<div class="gl-picking" role="status"><span>'+icon('scan')+'</span><div><strong>点击下面的目标窗口预览</strong><p>先核对窗口映射、进程与容器；点选本身不会启动接入。按 Esc 取消并保留当前会话。</p></div></div>';
    body+='<div class="gl-target-list '+(ui.picking?'gl-pick-grid':'')+'" aria-label="'+(ui.list==='windows'?'运行窗口':'进程列表')+'">'+windowChoices().map(item=>'<button type="button" class="gl-target '+(ui.selectedId===item.id?'selected':'')+'" aria-pressed="'+(ui.selectedId===item.id)+'" data-gl-action="select-target" data-gl-id="'+item.id+'">'+(ui.picking&&item.scene?'<img src="'+esc(item.scene)+'" alt="'+esc(item.title)+' 窗口预览">':'<span class="gl-target-icon">'+icon(ui.list==='windows'?'window':'activity')+'</span>')+'<div><strong>'+esc(ui.list==='windows'?item.title:item.exe)+'</strong><small>'+esc(item.self?'Luna 自身 · 不能作为 Hook 目标':item.bottle+' · '+item.arch+' · Windows PID '+item.windowsPid)+'</small></div><span class="gl-target-check">'+(ui.selectedId===item.id?icon('check'):'')+'</span></button>').join('')+'</div>';
    if(choice){
      body+='<section class="gl-mapping"><h3>关联并接入的目标</h3><dl><div><dt>窗口</dt><dd>'+esc(choice.title)+'</dd></div><div><dt>Windows 游戏进程</dt><dd>'+(choice.self?'不是 Windows 游戏进程':'PID '+choice.windowsPid+' · '+esc(choice.exe))+'</dd></div><div><dt>环境与容器</dt><dd>'+environmentName(choice.environment)+' · '+esc(choice.bottle)+'</dd></div><div><dt>目标完整路径</dt><dd><code>'+esc(choice.targetPath)+'</code></dd></div><div><dt>程序架构</dt><dd>'+esc(choice.arch)+'</dd></div></dl><details class="gl-runtime-identity"><summary>高级运行身份</summary><p>macOS 宿主 PID '+choice.hostPid+' · '+esc(choice.hostWindowId)+'</p><p>本次 Windows 进程创建标识：'+esc(choice.processStartedAt||'不适用')+'</p></details></section>';
      if(switching){const current=app.state.session.gameSnapshot||byId(app.state.session.gameId);body+='<div class="gl-switch-review" role="note"><strong>将切换取文目标</strong><p>当前：'+esc(current?.title||'当前游戏')+' · '+esc(current?.bottle||'')+'</p><p>切换到：'+esc(choice.title)+' · '+esc(choice.bottle)+'</p><p>确认后断开旧取文并启动新目标的 Hook；旧游戏与历史保留。取消不更改窗口关联、原句或当前会话。</p></div>';}
      body+='<p class="gl-identity-note '+(choice.self?'gl-mismatch':'')+'">'+icon(choice.self?'alert':'check')+'<span>'+(choice.self?'不能接入 Luna 自身。请选择游戏窗口或游戏进程。':reuse?'完整目标与本次运行身份一致，将复用当前 Hook，保留原句、正文选择和历史，不重复注入。':switching?'核对后点击“切换并接入”，这是本次切换的确认。':'确认后自动核验目标并启动 Hook，收到候选文字后再选择正文，无需另点连接。')+'</span></p>';
    }
    body+='<div class="gl-errors" role="alert" hidden></div></div>';return body;
  }
  function renderRunning(){
    const ui=store.running,app=store.app,choice=windowChoices().find(item=>item.id===ui.selectedId),switching=choice&&!choice.self&&app.state.session?.status==='connected'&&!sameRuntimeTarget(choice,app.state.session);
    app.dialog({title:ui.activeSession?'选择或更换游戏窗口':'选择正在运行的游戏',description:'选择窗口或进程，核对环境、完整路径与本次运行身份。确认关联时同步启动 Hook 接入。',body:runningForm(),confirmLabel:switching?'切换并接入':'关联并接入',cancelLabel:'取消',onConfirm:()=>{
      const selected=windowChoices().find(item=>item.id===ui.selectedId);
      if(!selected)return addError('请先选择一个目标并核对映射。');
      if(selected.self)return addError('不能接入 Luna 自身，请重新选择游戏窗口或进程。');
      let game=libraryGames().find(item=>identity(item)===identity(selected));
      if(!game){const source=store.catalogue.find(item=>item.id===selected.recordId);if(!source)return addError('尚未完整识别目标，请刷新后重新选择。');game=storeRecord(Object.assign(clone(source),{title:source.title}),source.id);}
      const reuse=sameRuntimeTarget(selected,app.state.session);
      store.bindings.clear();store.bindings.set(game.id,bindingFrom(selected,game.id));store.running=null;app.closeDialog();
      app.connectGame(game.id,{confirmed:true,targetWindow:clone(selected),replaceExisting:!!app.state.session&&app.state.session.status==='connected'&&!reuse});return false;
    }});
    const confirm=document.querySelector('#dialog-confirm');if(confirm)confirm.disabled=!choice||choice.self;
  }
  function openRunning(app,{gameId=null,launch=false}={}){
    const session=app.state.session,activeSession=!!session&&session.status==='connected'&&session.gameId===gameId&&!!session.gameSnapshot;
    const expectedGame=activeSession?session.gameSnapshot:byId(gameId);store.app=app;
    store.running={gameId,launch,expectedGame:expectedGame?clone(expectedGame):null,activeSession,selectedId:null,list:'windows',picking:false};renderRunning();
  }
  function addFiles(files){if(!store.add)return;collectCurrent();const incoming=Array.from(files||[]).filter(file=>/\.(exe|lnk|app)$/i.test(file.name||''));if(!incoming.length){addError('请选择 .exe、.lnk 或 .app 程序。文件内容不会被读取。');return;}incoming.forEach(file=>store.add.items.push(newRecord(file.name)));store.add.selected=store.add.items.length-incoming.length;renderAdd();}
  document.addEventListener('click',event=>{const button=event.target.closest('[data-gl-action]');if(!button)return;const action=button.dataset.glAction;
    if(action==='browse-files'){document.getElementById('gl-game-files').click();return;}
    if(action==='show-fixtures'){collectCurrent();store.add.showFixtures=!store.add.showFixtures;renderAdd();return;}
    if(action==='add-fixture'){collectCurrent();const fixture=store.catalogue.find(item=>item.id===button.dataset.glId);store.add.items.push(clone(fixture));store.add.selected=store.add.items.length-1;store.add.showFixtures=false;renderAdd();return;}
    if(action==='select-item'){collectCurrent();store.add.selected=Number(button.dataset.glIndex);renderAdd();return;}
    if(action==='running-tab'){store.running.list=button.dataset.glValue;store.running.picking=false;renderRunning();return;}
    if(action==='pick-window'){store.running.picking=true;store.running.list='windows';store.running.selectedId=null;renderRunning();return;}
    if(action==='select-target'){store.running.selectedId=button.dataset.glId;store.running.picking=false;renderRunning();return;}
    if(action==='clear-binding'){const id=store.running.gameId;store.running=null;store.app.closeDialog();unbind(store.app,id);return;}
  });
  document.addEventListener('change',event=>{if(event.target.id==='gl-game-files')addFiles(event.target.files);});
  document.addEventListener('input',event=>{if(!event.target.matches('[data-gl-field]')||!store.add)return;collectCurrent();const error=document.querySelector('#app-dialog .gl-errors');if(error){error.hidden=true;error.textContent='';}const note=document.querySelector('#app-dialog .gl-add .gl-identity-note'),record=store.add.items[store.add.selected];if(note&&record)note.innerHTML=fixtureFor(record)?icon('check')+'<span>完整身份匹配既有游戏设计素材，可继续体验该游戏的样例流程。</span>':icon('alert')+'<span>此程序没有已匹配的采集素材。保存后可建库；不会显示其他游戏的原文或假装取文成功。</span>';});
  document.addEventListener('dragover',event=>{if(event.target.closest('[data-gl-drop]')){event.preventDefault();event.dataTransfer.dropEffect='copy';}});
  document.addEventListener('drop',event=>{if(event.target.closest('[data-gl-drop]')){event.preventDefault();addFiles(event.dataTransfer.files);}});
  document.addEventListener('keydown',event=>{if((event.key==='Enter'||event.key===' ')&&event.target.matches('[data-gl-drop]')){event.preventDefault();document.getElementById('gl-game-files').click();}});
  window.LunaLibrary={initialise,allGames,libraryGames,openAdd,openEdit,remove,openRunning,getBinding,unbind,onConnected,onGameExit,isRunning,windowChoices,sameRuntimeTarget};
})();
