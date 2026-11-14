/* Independent design replay. No React import, IPC, native API, network or storage. */
(() => {
 'use strict';
 const params=new URLSearchParams(location.search);
 const requested=params.get('state')||'normal';
 const names=['normal','empty','loading','loading-initial','read-error','save-failed'];
 const state=names.includes(requested)?requested:'normal';
 document.documentElement.dataset.context=params.get('context')==='native-proposal'?'native-proposal':'web';
 document.body.dataset.candidateState=state;
 const content=document.querySelector('.content');
 const scope=document.querySelector('#scope-button');
 const save=document.querySelector('#save');
 const discard=document.querySelector('#discard');
 const status=document.querySelector('#save-status');
 const fields=[...document.querySelectorAll('[data-field]')];
 const notice=document.createElement('div');notice.className='state-notice';notice.id='state-notice';notice.hidden=true;
 content.querySelector('.heading').after(notice);
 let pending=false,saving=false;
 const setStatus=text=>{status.replaceChildren(document.createTextNode(text));const s=document.createElement('span');s.className='status-scope';s.textContent='全局默认';status.append(s);};
 const noticeText=(message,error=false,retry=false)=>{
  notice.hidden=false;notice.classList.toggle('error',error);notice.setAttribute('role',error?'alert':'status');notice.replaceChildren();
  if(!error){const mark=document.createElement('span');mark.className='pending-mark';mark.setAttribute('aria-hidden','true');notice.append(mark);}
  const p=document.createElement('p');p.textContent=message;notice.append(p);
  if(retry){const b=document.createElement('button');b.className='control';b.id='retry-source';b.textContent='重新读取偏好来源';b.addEventListener('click',()=>{pending=true;render();setTimeout(()=>{pending=false;render();document.querySelector('#retry-source')?.focus({preventScroll:true});},900);});notice.append(b);}
 };
 function render(){
  const dirty=window.SettingsStructureCandidate.state.dirty;
  const loading=state==='loading'||state==='loading-initial'||pending;
  content.setAttribute('aria-busy',String(loading));
  if(loading){noticeText('正在读取已保存的阅读偏好来源…');setStatus('正在核对阅读偏好');}
  else if(state==='read-error'){noticeText('阅读偏好来源读取失败：暂时无法读取已保存的偏好。已显示的编辑与已保存配置保留。',true,true);setStatus('阅读偏好来源待确认');}
  else if(state==='save-failed'&&dirty){noticeText('阅读偏好保存失败，请重试。编辑内容已保留。',true);setStatus('阅读偏好有未保存修改');}
  else notice.hidden=true;
  if(loading||state==='read-error'){fields.forEach(b=>b.disabled=true);scope.disabled=true;save.disabled=true;discard.disabled=!dirty||loading;}
  else if(state==='save-failed'&&dirty){scope.disabled=true;save.disabled=false;discard.disabled=false;}
  if(saving){noticeText('正在保存阅读偏好…');setStatus('正在保存阅读偏好');save.disabled=true;discard.disabled=true;}
  document.body.dataset.phase=saving?'saving':loading?'loading':state;
 }
 if(state==='loading-initial'){document.querySelector('.lexical-card').closest('.group').hidden=true;document.querySelector('#scope-help').textContent='阅读偏好来源待确认';}
 if(state==='empty')document.querySelector('[data-scope="game"]').remove();
 // Read/save failures demonstrate retention using one unsaved POS edit.
 // It is the same V3 in-memory editing interaction, not a product fixture.
 if(state==='read-error'||state==='save-failed')document.querySelector('#pos-switch').click();
 document.addEventListener('click',event=>{
  const target=event.target.closest('button');if(!target)return;
  if(target===save&&state==='save-failed'){
   event.preventDefault();event.stopImmediatePropagation();saving=true;save.disabled=true;discard.disabled=true;
   noticeText('正在保存阅读偏好…');setStatus('正在保存阅读偏好');
   setTimeout(()=>{saving=false;render();save.focus({preventScroll:true});},900);return;
  }
  if(target===discard&&state==='read-error'){event.preventDefault();event.stopImmediatePropagation();window.SettingsStructureCandidate.reset();render();return;}
  if(target===discard||target.matches('[data-field]'))setTimeout(render,0);
 },true);
 window.GeneralStateGapCandidate={kind:'independent-design-pending-user-review',state,context:document.documentElement.dataset.context,get draft(){return window.SettingsStructureCandidate.state;},nativeVerified:false,referenceApproved:false};
 render();
})();
