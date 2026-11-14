/* Review controls live outside the product frame. Reloading here intentionally
   chooses an independent design scenario, not a production state transition. */
(() => {
  const query=new URLSearchParams(location.search);
  let page=['history','settings','session'].includes(query.get('page'))?query.get('page'):'history',compare='after';
  const notes={history:['筛选与两栏对齐','顶部游戏、会话筛选与下方列表、详情共用列轨和16px内容线。“加载更早会话”留在会话范围内，保留轻量层级。'],settings:['每行带自己的安全边距','开关标签与说明在左，开关在右；行内距、组标题和卡片间距统一。历史显示始终保存状态，不恢复可关闭开关。'],session:['已停止取文，仍可继续阅读','断开时只保留复制、重译、精读三个旧句动作；页头突出重新连接。未关联的空预览不再占右栏；有效预览仍独立保留。']};
  const frame=document.getElementById('design-frame');
  function render(){
    const narrow=document.getElementById('review-size').value==='narrow',theme=document.getElementById('review-theme').value,mode=document.getElementById('review-connection').value;
    const width=narrow?760:page==='settings'?1000:1040,height=narrow?560:720;
    const q=new URLSearchParams({scenario:page==='history'?'history':page==='settings'?'settings':'reading',theme,settingsSize:'native',compare});
    if(page==='session'){q.set('connection',mode==='verified'?'disconnected':mode);q.set('window',mode==='verified'?'verified':'none');if(mode==='disconnected')q.set('translation','failed');}
    const url=page+'.html?'+q;frame.width=String(width);frame.height=String(height);frame.src=url;
    document.getElementById('open-page').href=url;document.getElementById('session-choice').hidden=page!=='session';
    document.getElementById('change-title').textContent=notes[page][0];document.getElementById('change-description').textContent=notes[page][1];document.getElementById('window-size').textContent=width+' × '+height+' 逻辑像素 · '+(compare==='after'?'待确认修订':'冻结父源对照')+' · 所有审阅控件均位于产品外';
    document.querySelectorAll('[data-review-page]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.reviewPage===page)));document.querySelectorAll('[data-compare]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.compare===compare)));
    const locationQuery=new URLSearchParams(location.search);locationQuery.set('page',page);history.replaceState(null,'','?'+locationQuery);
  }
  document.querySelectorAll('[data-review-page]').forEach(b=>b.addEventListener('click',()=>{page=b.dataset.reviewPage;render();}));document.querySelectorAll('[data-compare]').forEach(b=>b.addEventListener('click',()=>{compare=b.dataset.compare;render();}));document.querySelectorAll('.canvas-tools select').forEach(s=>s.addEventListener('change',render));render();
})();
