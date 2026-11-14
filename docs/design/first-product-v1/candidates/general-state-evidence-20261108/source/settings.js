/* Memory-only candidate interactions. No native command, HTTP, SQL or storage. */
(() => {
 'use strict';
 window.LunaIcons.mount(document);
 const q=new URLSearchParams(location.search),html=document.documentElement;
 html.dataset.theme=q.get('theme')==='light'?'light':'dark';
 const zoom=q.get('zoom')==='2'?2:1;html.style.zoom=String(zoom);
 // Keep the window's available CSS height at the requested physical viewport.
 document.querySelector('.window').style.height=`${100/zoom}dvh`;
 const initial={segmentation:true,ruby:true,pos:false};
 const saved={global:{...initial},game:{overrides:{}}},state={scope:'global',draft:{...initial},draftOverrides:{},dirty:false};
 const fieldNames={segmentation:'日语分词',ruby:'日语注音',pos:'词性标记'};
 const effectiveGame=()=>({...saved.global,...saved.game.overrides});
 const stable=value=>JSON.stringify(Object.entries(value).sort(([a],[b])=>a.localeCompare(b)));
 function loadDraft(){state.draft=state.scope==='global'?{...saved.global}:effectiveGame();state.draftOverrides=state.scope==='global'?{}:{...saved.game.overrides};}
 const buttons=[...document.querySelectorAll('[data-field]')],dialog=document.querySelector('#sample-dialog'),scopeButton=document.querySelector('#scope-button'),scopeOptions=document.querySelector('#scope-options');
 // The standalone review canvas uses a body-level menu so the content scroll
 // viewport and fixed footer cannot clip an option. Production reuses C04.
 document.body.append(scopeOptions);
 function placeScope(){
  const scale=parseFloat(getComputedStyle(html).zoom)||1,rect=scopeButton.getBoundingClientRect(),safe=12;
  const physicalWidth=Math.min(Math.max(rect.width,220*scale),innerWidth-safe*2);
  scopeOptions.style.width=physicalWidth/scale+'px';scopeOptions.style.maxHeight=(innerHeight-safe*2)/scale+'px';scopeOptions.style.left='0';scopeOptions.style.top='0';
  const height=scopeOptions.getBoundingClientRect().height,below=innerHeight-safe-rect.bottom,above=rect.top-safe;
  const preferred=height>below&&above>below?rect.top-5*scale-height:rect.bottom+5*scale;
  const left=Math.max(safe,Math.min(rect.left,innerWidth-safe-physicalWidth)),top=Math.max(safe,Math.min(preferred,innerHeight-safe-height));
  scopeOptions.style.left=left/scale+'px';scopeOptions.style.top=top/scale+'px';
 }
 window.addEventListener('resize',()=>{if(!scopeOptions.hidden)placeScope();});
 let toastTimer;
 function toast(text){const el=document.querySelector('#toast');el.textContent=text;el.hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>{el.hidden=true;},2600);}
 function show(title,text){document.querySelector('#dialog-title').textContent=title;document.querySelector('#dialog-body').textContent=text;dialog.showModal();}
 function closeScope(){scopeOptions.hidden=true;scopeButton.setAttribute('aria-expanded','false');}
 function render(){
  state.dirty=state.scope==='global'?stable(state.draft)!==stable(saved.global):stable(state.draftOverrides)!==stable(saved.game.overrides);
  buttons.forEach(button=>{button.setAttribute('aria-checked',String(state.draft[button.dataset.field]));button.disabled=button.dataset.field==='pos'&&!state.draft.segmentation;});
  document.querySelector('#pos-description').textContent=state.draft.segmentation?'在可点击分词开启时显示。':'先开启可点击分词；当前词性偏好保持不变。';
  document.querySelector('#save').disabled=!state.dirty;document.querySelector('#discard').disabled=!state.dirty;scopeButton.disabled=state.dirty;
  const scope=state.scope==='global'?'全局默认':'仅此游戏 · サクラノ詩';
  document.querySelector('#scope-value').textContent=scope;
  document.querySelector('#scope-help').textContent=state.dirty?'先保存或放弃修改，再切换范围':state.scope==='global'?'作为游戏的默认阅读偏好':'修改只作用于此游戏';
  const gameScope=state.scope==='game',savedFields=Object.keys(saved.game.overrides),draftFields=Object.keys(state.draftOverrides);
  document.querySelector('#game-source').hidden=!gameScope;
  document.querySelector('#current-source').textContent='当前来源：'+(savedFields.length?'游戏覆盖 · '+savedFields.length+'项':'继承全局');
  document.querySelector('#restore-inheritance').disabled=!draftFields.length;
  const draftHint=document.querySelector('#inheritance-draft');draftHint.hidden=!gameScope||!state.dirty;draftHint.textContent=draftFields.length?'草稿：覆盖'+draftFields.map(field=>fieldNames[field]).join('、')+'，保存后生效':'草稿：恢复继承全局，保存后生效';
  document.querySelector('#saved-overrides').hidden=!gameScope||!savedFields.length;
  document.querySelector('#saved-overrides-list').textContent=savedFields.map(field=>fieldNames[field]).join('、');
  const status=document.querySelector('#save-status');status.replaceChildren(document.createTextNode(state.dirty?'阅读偏好有未保存修改':'阅读偏好已保存'));const small=document.createElement('span');small.className='status-scope';small.textContent=scope;status.append(small);
  scopeOptions.querySelectorAll('[data-scope]').forEach(button=>button.setAttribute('aria-selected',String(button.dataset.scope===state.scope)));
 }
 buttons.forEach(button=>button.addEventListener('click',()=>{const field=button.dataset.field;state.draft[field]=!state.draft[field];if(state.scope==='game')state.draftOverrides[field]=state.draft[field];render();}));
 document.querySelector('#save').addEventListener('click',()=>{if(state.scope==='global')saved.global={...state.draft};else saved.game.overrides={...state.draftOverrides};loadDraft();render();toast('设计样本：已保存此范围的阅读偏好');});
 document.querySelector('#discard').addEventListener('click',()=>{loadDraft();render();toast('已恢复此范围的已保存偏好');});
 document.querySelector('#restore-inheritance').addEventListener('click',()=>{state.draft={...saved.global};state.draftOverrides={};render();});
 scopeButton.addEventListener('click',()=>{const open=scopeOptions.hidden;scopeOptions.hidden=!open;scopeButton.setAttribute('aria-expanded',String(open));if(open){placeScope();scopeOptions.querySelector('[aria-selected=true]').focus();}});
 scopeOptions.addEventListener('click',event=>{const button=event.target.closest('[data-scope]');if(!button)return;state.scope=button.dataset.scope;loadDraft();closeScope();render();scopeButton.focus();});
 scopeOptions.addEventListener('keydown',event=>{const options=[...scopeOptions.querySelectorAll('button')],current=options.indexOf(document.activeElement);let next;if(event.key==='ArrowDown')next=(current+1)%options.length;if(event.key==='ArrowUp')next=(current+options.length-1)%options.length;if(event.key==='Home')next=0;if(event.key==='End')next=options.length-1;if(next!==undefined){event.preventDefault();options[next].focus();}if(event.key==='Escape'){event.preventDefault();closeScope();scopeButton.focus();}});
 document.addEventListener('click',event=>{if(!event.target.closest('.scope-select')&&!scopeOptions.contains(event.target))closeScope();});
 const detailButton=document.querySelector('#login-details-toggle'),detail=document.querySelector('#login-details');detailButton.addEventListener('click',()=>{detail.hidden=!detail.hidden;detailButton.setAttribute('aria-expanded',String(!detail.hidden));detailButton.textContent=detail.hidden?'查看原因':'收起原因';});
 const actions={main:()=>show('返回主界面','设计回放：返回现有 Luna 主窗口，不关闭当前会话。本候选只调整通用设置；当前客户端不会被此网页操作。'),history:()=>show('阅读历史','设计回放：打开主窗口“历史记录”，按游戏和会话查看已保存的原文、译文及来源。历史始终保存，没有关闭开关。'),session:()=>show('当前会话','设计回放：打开主窗口“当前会话”。自动输入、自动翻译、暂停与恢复在对应的游戏会话中操作；通用设置不再重复提供会话开关。'),'retry-login':()=>{document.querySelector('#login-result').textContent='样本回执：系统仍未找到应用服务，状态未改变。';},'close-dialog':()=>dialog.close()};
 document.addEventListener('click',event=>{const button=event.target.closest('[data-action]');if(button)actions[button.dataset.action]?.();});
 document.querySelectorAll('[data-page]').forEach(button=>button.addEventListener('click',()=>{if(button.dataset.page==='general'){document.querySelector('.content').scrollTo({top:0,behavior:'auto'});toast('已回到通用设置');}else show(button.textContent.trim(),'本候选仅重排“通用”设置，其余分类沿用现行已确认设计，未在这里新增或替代功能。');}));
 document.querySelector('input[type=search]').addEventListener('input',event=>{const value=event.target.value.trim().toLowerCase();document.querySelectorAll('[data-page]').forEach(button=>button.hidden=value&&!button.textContent.toLowerCase().includes(value));});
 window.SettingsStructureCandidate={kind:'design-only-pending-review',get state(){return structuredClone(state);},get saved(){return structuredClone(saved);},get effectiveGame(){return effectiveGame();},reset(){state.scope='global';saved.global={...initial};saved.game.overrides={};loadDraft();clearTimeout(toastTimer);document.querySelector('#toast').hidden=true;closeScope();detail.hidden=true;detailButton.setAttribute('aria-expanded','false');detailButton.textContent='查看原因';document.querySelector('#login-result').textContent='';render();}};
 render();
})();
