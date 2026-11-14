/* Design-only composition. Existing prototype owns settings, history and actions.
   This file does not import production code or call native/network/storage APIs. */
(() => {
  'use strict';
  const app=window.LunaDesign,gaps=window.LunaReviewGaps,q=new URLSearchParams(location.search);
  const screen=document.documentElement.dataset.layoutRefinement;
  const before=q.get('compare')==='before';
  document.documentElement.dataset.layoutCompare=before?'before':'after';
  if(before)document.querySelectorAll('link[data-layout-delta]').forEach(node=>node.remove());
  const stopped=()=>app.state.session.gameId&&app.state.session.status!=='connected';
  const hasVerifiedWindow=()=>{const game=app.game();return !!(game&&app.state.session.gameId&&window.LunaLibrary?.getBinding(app.state.session.gameId,game));};
  const parentSession=gaps.sessionHTML;
  if(!before)gaps.sessionHTML=function(target,readingHTML){
    const original=parentSession(target,readingHTML);
    if(!stopped())return original;
    const box=document.createElement('div');box.innerHTML=original;
    const heading=box.querySelector('.page-header'),layout=box.querySelector('.guided-reading-layout');
    if(!heading||!layout)return original;
    heading.classList.add('layout-stopped-heading');layout.classList.add('layout-stopped-reading');
    if(hasVerifiedWindow())layout.dataset.verifiedWindow='true';
    const description=heading.querySelector('p');
    if(description)description.textContent=(app.state.session.status==='exited'?'游戏已退出':'游戏取文已断开')+' · 已保存的原文、译文和查词仍可使用';
    heading.querySelector('.page-actions')?.remove();
    const actions=document.createElement('div');actions.className='layout-recovery-actions';
    actions.innerHTML='<button type="button" data-action="reconnect" class="primary">重新连接游戏…</button><button type="button" data-action="show-hud">显示浮窗</button>';
    heading.append(actions);
    layout.querySelectorAll('.status-banner').forEach(node=>node.remove());
    if(!hasVerifiedWindow())layout.querySelector('.rw-preview-column')?.remove();
    const status=layout.querySelector('.guided-live-status');
    if(status){status.classList.add('layout-saved-status');status.innerHTML='<span>最近保存的一句</span><small>已停止接收新台词</small>';}
    const card=layout.querySelector('.reading-unit');
    if(card){card.classList.add('layout-last-sentence');card.dataset.translationState=app.state.current?.translationStatus||'none';const label=card.querySelector('.unit-meta span:last-child');if(label)label.textContent='上次会话 · 非实时';}
    if(card&&app.state.current?.translationStatus==='disabled'){
      const message=card.querySelector('.translation');if(message)message.textContent='当前翻译服务不可用，原文已保留';
      const help=document.createElement('div');help.className='layout-translation-recovery';help.innerHTML='<span>检查翻译服务的配置与启用状态后，可重新翻译这句。</span><button type="button" data-action="open-translation-settings">翻译设置…</button>';card.append(help);
    }
    const controls=layout.querySelector('.session-controls');
    if(controls){
      controls.classList.add('layout-saved-actions');
      controls.querySelectorAll('button').forEach(button=>{
        if(!['copy-current','retranslate-current','learn-current'].includes(button.dataset.action))button.remove();
        else if(!app.state.current)button.remove();
      });
      const learn=controls.querySelector('[data-action="learn-current"]');if(learn)learn.textContent='精读这句';
    }
    if(!app.state.current){
      if(status)status.remove();
      const empty=document.createElement('div');empty.className='layout-session-empty';empty.innerHTML='<h2>当前没有可显示的句子</h2><p>已保存内容可从下方正文历史或“历史记录”回看；连接已运行的游戏后继续接收新句。</p>';
      layout.querySelector('#flat-reading')?.append(empty);
    }
    // Binding/preview remains an independent capability. An active verified
    // preview is retained; an unbound placeholder is not a second recovery CTA.
    if(!hasVerifiedWindow())box.querySelector('.flat-preview-management')?.remove();
    return box.innerHTML;
  };
  function historyLayout(){
    const filters=document.querySelector('.hx-filters');if(!filters||filters.querySelector('.layout-history-session-tools'))return;
    const labels=[...filters.children].filter(node=>node.classList.contains('hx-filter'));
    const directory=filters.querySelector('.hx-directory');if(labels.length!==2||!directory)return;
    const wrapper=document.createElement('div');wrapper.className='layout-history-session-tools';
    filters.insertBefore(wrapper,labels[1]);wrapper.append(labels[1],directory);
  }
  const toggleDetails={
    'general.segmentation':['可点击日语分词','关闭分词仍可独立显示真实注音。'],
    'general.furigana':['显示日语注音','使用本机分词资源；复制原文不包含注音。'],
    'general.pos':['显示词性标记','在可点击分词开启时呈现；关闭分词不会丢失词性偏好。']
  };
  function settingsLayout(){
    const category=app.state.settings.category;
    if(['ocr','speech'].includes(category)){
      const row=document.querySelector('#settings-content .ls-row');
      const label=row?.querySelector('.ls-row-label');if(label&&label.textContent!=='后续范围')label.textContent='后续范围';
      const value=row?.querySelector('.ls-row-value');
      const description='OCR文字识别属于后续范围；本期游戏窗口预览独立于OCR，按系统要求申请屏幕录制权限。';
      if(category==='ocr'&&value&&value.textContent!==description)value.textContent=description;
    }
    if(category!=='general')return;
    document.querySelectorAll('[data-settings-field]').forEach(input=>{
      const detail=toggleDetails[input.dataset.settingsField];if(!detail)return;
      const row=input.closest('.ls-row');if(!row||row.classList.contains('layout-toggle-row'))return;
      row.classList.add('layout-toggle-row');
      const label=row.querySelector('.ls-row-label');if(label)label.textContent=detail[0];
      input.setAttribute('role','switch');input.setAttribute('aria-label',detail[0]);
      if(input.dataset.settingsField==='general.pos')input.disabled=!app.state.settings.general.segmentation;
      const check=input.closest('.ls-check');if(check){check.setAttribute('aria-label',detail[0]);const text=check.querySelector('span');if(text)text.textContent='';}
      const note=document.createElement('div');note.className='ls-help';note.textContent=detail[1];label?.append(note);
    });
    const oldHistory=document.querySelector('[data-settings-field="general.history"]');
    if(oldHistory){const row=oldHistory.closest('.ls-row');const value=row?.querySelector('.ls-row-value');if(value)value.innerHTML='<span class="ls-badge">始终保存原文、译文及来源</span><div class="ls-help">保存独立于显示开关；可从主窗口“历史记录”查看。</div>';}
  }
  let queued=false;
  function auxiliaryCanvas(){
    const aux=!app.state.windows.settings&&app.state.windows.learning?'learning':!app.state.windows.settings&&app.state.windows.hud?'hud':'none';
    if(document.documentElement.dataset.layoutAux!==aux)document.documentElement.dataset.layoutAux=aux;
  }
  const decorate=()=>{queued=false;if(before)return;historyLayout();settingsLayout();auxiliaryCanvas();};
  const observer=new MutationObserver(()=>{if(!queued){queued=true;queueMicrotask(decorate);}});
  observer.observe(document.getElementById('main-content'),{childList:true,subtree:true});
  observer.observe(document.getElementById('settings-content'),{childList:true,subtree:true});
  const parentAfter=gaps.afterRender;
  gaps.afterRender=function(...args){const result=parentAfter.apply(this,args);decorate();return result;};
  app.state.windows.hud=false;app.state.windows.game=false;app.state.windows.learning=false;
  if(screen==='settings'){
    app.state.settings.category='general';app.state.settings.notice='阅读偏好已保存 · 修改不会重新接入游戏';app.state.windows.settings=true;app.state.windows.main=false;
    document.documentElement.dataset.clientRole='settings';
  }else{
    app.state.windows.settings=false;app.state.windows.main=true;app.state.page=screen==='history'?'history':'session';
    document.documentElement.dataset.clientRole='main';
    if(screen==='session'){
      const mode=q.get('connection')||'disconnected';
      if(mode!=='connected')app.disconnect(false);
      if(mode!=='connected'&&q.get('window')!=='verified'){
        window.LunaLibrary?.unbind(app,app.state.session.gameId);
        // Initializing a review fixture is not a user action. Only hide this
        // startup toast; subsequent real prototype unbind actions still notify.
        const toast=document.getElementById('app-toast');if(toast)toast.hidden=true;
      }
      if(mode==='exited')app.state.session.status='exited';
      if(mode==='empty')app.state.current=null;
      if(app.state.current&&q.get('translation')==='failed'){
        app.state.current.translation='';app.state.current.editedTranslation='';app.state.current.translationStatus='failed';
      }
    }
  }
  app.render();decorate();
  window.LunaLayoutRefinement={kind:'design-only',screen,before,stopped:stopped(),get verifiedWindow(){return hasVerifiedWindow();}};
})();
