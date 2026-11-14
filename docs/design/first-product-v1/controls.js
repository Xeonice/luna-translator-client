/* Shared presentation-only Apple-style listbox. Hidden select retains field data;
   the visible trigger, panel, focus and keyboard interaction are authored here. */
(() => {
  'use strict';
  let sequence = 0, active = null, queued = false;
  const icon = name => window.LunaIcons?.svg(name) || '';
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  function close(restore = false) {
    if (!active) return;
    const prior = active; active = null;
    prior.panel.remove(); prior.trigger.setAttribute('aria-expanded', 'false');
    prior.trigger.removeAttribute('aria-activedescendant');
    if (restore && prior.trigger.isConnected) prior.trigger.focus({preventScroll:true});
  }
  function sync(select) {
    const trigger = select.nextElementSibling;
    if (!trigger?.classList.contains('luna-select-trigger')) return;
    const label = select.selectedOptions[0]?.textContent || select.getAttribute('data-placeholder') || '请选择';
    trigger.innerHTML = '<span class="luna-select-value">' + esc(label) + '</span><span class="luna-select-chevron" aria-hidden="true">' + icon('chevron-down') + '</span>';
    trigger.disabled = select.disabled; trigger.title = label;trigger.dataset.selectState=select.dataset.selectState||'ready';if(select.dataset.selectState==='error')trigger.setAttribute('aria-invalid','true');else trigger.removeAttribute('aria-invalid');
    trigger.setAttribute('aria-label', (select.getAttribute('aria-label') || select.closest('label')?.querySelector('span')?.textContent || '选择') + '：' + label);
  }
  function upgrade(root = document) {
    root.querySelectorAll('select').forEach(select => {
      if (select.closest('.review-bar') || select.dataset.customSelect === 'ready') return;
      if (!select.id) select.id = 'design-select-' + (select.dataset.settingsField || select.dataset.glField || ++sequence).replace(/[^\w-]/g,'-');
      select.dataset.customSelect = 'ready';
      const wrapper = document.createElement('span'); wrapper.className = 'luna-select';
      select.replaceWith(wrapper); wrapper.append(select); select.hidden = true; select.tabIndex = -1;
      const trigger = document.createElement('button'); trigger.type = 'button'; trigger.className = 'luna-select-trigger';
      trigger.dataset.selectFor = select.id; trigger.id = select.id + '--trigger';
      trigger.setAttribute('role','combobox'); trigger.setAttribute('aria-haspopup','listbox'); trigger.setAttribute('aria-expanded','false');
      trigger.setAttribute('aria-controls', select.id + '--listbox');
      wrapper.append(trigger); sync(select);
    });
    if (active && !active.trigger.isConnected) close();
  }
  function focusOption(index) {
    if (!active) return;
    const enabled = [...active.panel.querySelectorAll('[role=option]:not([aria-disabled=true])')];
    if (!enabled.length) return;
    const option = enabled[(index + enabled.length) % enabled.length];
    active.index = enabled.indexOf(option);
    active.panel.querySelectorAll('[role=option]').forEach(el => el.classList.toggle('is-active',el === option));
    active.trigger.setAttribute('aria-activedescendant',option.id);
    // Scroll only this popup. scrollIntoView also scrolls a containing dialog,
    // which can dismiss a just-opened popup before a released click arrives.
    const top=option.offsetTop,bottom=top+option.offsetHeight;
    if(top<active.panel.scrollTop)active.panel.scrollTop=top;
    else if(bottom>active.panel.scrollTop+active.panel.clientHeight)active.panel.scrollTop=bottom-active.panel.clientHeight;
  }
  function open(trigger) {
    if (trigger.disabled) return;
    if (active?.trigger === trigger) { close(); return; }
    close();
    const select = document.getElementById(trigger.dataset.selectFor);
    if (!select) return;
    const panel = document.createElement('div'); panel.id = select.id + '--listbox'; panel.className = 'luna-select-panel';
    panel.setAttribute('role','listbox'); panel.setAttribute('aria-label',select.getAttribute('aria-label') || '选择');
    panel.innerHTML = [...select.options].map((option,index) => '<button type="button" role="option" tabindex="-1" id="'+esc(select.id)+'--option-'+index+'" data-value="'+esc(option.value)+'" aria-selected="'+option.selected+'" aria-disabled="'+option.disabled+'"><span class="luna-option-check" aria-hidden="true">'+(option.selected?icon('check'):'')+'</span><span>'+esc(option.textContent)+'</span></button>').join('') || '<div class="luna-select-empty" role="status">暂无可选项</div>';
    (select.closest('dialog') || document.body).append(panel);
    const box = trigger.getBoundingClientRect(), desired = Math.min(340, Math.max(box.width,220));
    panel.style.width = Math.min(desired,window.innerWidth - 24) + 'px';
    panel.style.left = Math.max(12, Math.min(box.left, window.innerWidth - desired - 12)) + 'px';
    const height = Math.min(286,panel.scrollHeight), below = window.innerHeight - box.bottom;
    panel.style.top = (below > height + 12 ? box.bottom + 5 : Math.max(12,box.top - height - 5)) + 'px';
    active = {trigger,select,panel,index:0}; trigger.setAttribute('aria-expanded','true'); trigger.focus({preventScroll:true});
    focusOption([...panel.querySelectorAll('[role=option]:not([aria-disabled=true])')].findIndex(el=>el.getAttribute('aria-selected')==='true'));
  }
  function choose(option) {
    if (!active || !option || option.getAttribute('aria-disabled') === 'true') return;
    const {select,trigger} = active; select.value = option.dataset.value; close(); sync(select);
    select.dispatchEvent(new Event('input',{bubbles:true})); select.dispatchEvent(new Event('change',{bubbles:true}));
    const replacement = document.getElementById(trigger.id); replacement?.focus({preventScroll:true});
  }
  document.addEventListener('pointermove',event => {
    const option = event.target.closest('.luna-select-panel [role=option]');
    if (!active || !option || !active.panel.contains(option) || option.getAttribute('aria-disabled') === 'true') return;
    const enabled = [...active.panel.querySelectorAll('[role=option]:not([aria-disabled=true])')];
    const index = enabled.indexOf(option);
    if (index >= 0 && index !== active.index) focusOption(index);
  });
  document.addEventListener('click',event => {
    const trigger = event.target.closest('.luna-select-trigger'); if (trigger) { event.preventDefault(); open(trigger); return; }
    const option = event.target.closest('.luna-select-panel [role=option]'); if (option) { choose(option); return; }
    if (active && !event.target.closest('.luna-select-panel')) close();
  });
  document.addEventListener('keydown',event => {
    const trigger = event.target.closest('.luna-select-trigger'); if (!trigger) return;
    if (['ArrowDown','ArrowUp','Enter',' ','Escape','Home','End'].includes(event.key)) {
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.key === 'Escape') { close(true); return; }
      if (!active) { open(trigger); return; }
      const options = active.panel.querySelectorAll('[role=option]:not([aria-disabled=true])');
      if (event.key === 'Enter' || event.key === ' ') choose(options[active.index]);
      else focusOption(event.key==='Home'?0:event.key==='End'?options.length-1:active.index+(event.key==='ArrowUp'?-1:1));
    } else if (event.key === 'Tab') close();
    else if (active && event.key.length === 1 && !event.metaKey && !event.ctrlKey) {
      const options = [...active.panel.querySelectorAll('[role=option]:not([aria-disabled=true])')];
      const match = options.findIndex(option=>option.textContent.trim().toLowerCase().startsWith(event.key.toLowerCase()));
      if (match>=0) focusOption(match);
    }
  },true);
  document.addEventListener('change',event=>{if(event.target.matches('select[data-custom-select]'))sync(event.target);});
  window.addEventListener('resize',()=>close());
  document.addEventListener('wheel',event=>{if(active&&!event.target.closest?.('.luna-select-panel'))close();},{passive:true});
  document.addEventListener('touchmove',event=>{if(active&&!event.target.closest?.('.luna-select-panel'))close();},{passive:true});
  new MutationObserver(()=>{if(queued)return;queued=true;queueMicrotask(()=>{queued=false;upgrade();});}).observe(document.body,{childList:true,subtree:true});
  window.LunaControls = {upgrade,close,sync}; upgrade();
})();
