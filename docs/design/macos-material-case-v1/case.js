(function(){
  'use strict';
  const paths={
    brand:'<path d="M3 4.5 10 7v11l-7-2.5zM10 7l10-2.5v9L10 16M7 11.5l7-2.5v11l-7-1.5z"/>',
    capture:'<path d="M5 8V4h4M15 4h4v4M19 16v4h-4M9 20H5v-4M8 16l4-9 4 9M9.5 13h5"/>',
    model:'<path d="M12 4c3-3 7 0 6 3 5 0 6 6 2 8 2 4-3 8-6 5-3 4-8 1-8-2-5 0-7-6-3-8-2-5 4-9 7-6M12 4v9l6 3M6 18l7-4V7M3 10l8 4 7-4M10 4l7 4v8l-7 4M6 6v8l8 5"/>',
    wand:'<path d="m4 20 13-13 3 3L7 23zM14 4V1M19 5l2-2M21 8h3M7 5V2M5.5 3.5h3M10 13l3 3"/>',
    pin:'<path d="m8 3 8 0-1 6 3 4H6l3-4zM12 13v8" fill="currentColor" stroke-linejoin="round"/>',
    ocr:'<path d="M4 7V4h4M4 15v4h4M16 4h4v4M20 16v4h-4M9 8h7M12.5 8v9M8 12h5"/>',
    speaker:'<path d="M4 9h4l5-4v14l-5-4H4zM16 8c2 2 2 6 0 8M19 5c4 4 4 10 0 14"/>',
    chevron:'<path d="m5 9 7 7 7-7"/>',
    sliders:'<path d="M4 7h4M13 7h7M4 17h9M18 17h2"/><circle cx="10.5" cy="7" r="2.5"/><circle cx="15.5" cy="17" r="2.5"/>',
    history:'<path d="M4 8a8 8 0 1 1-1 7M4 3v5h5M12 7v6l4 2"/>'
  };
  document.querySelectorAll('[data-icon]').forEach(node=>{node.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'+paths[node.dataset.icon]+'</svg>';});
  const stage=document.getElementById('case-stage'),viewport=document.getElementById('stage-viewport'),panel=document.getElementById('material-window');
  let scale=1,drag=null;
  function fit(){scale=Math.min(1,viewport.clientWidth/1462);stage.style.transform='scale('+scale+')';viewport.style.height=(920*scale)+'px';}
  addEventListener('resize',fit);fit();
  document.getElementById('toggle-window').addEventListener('click',event=>{panel.hidden=!panel.hidden;event.currentTarget.textContent=panel.hidden?'显示窗口':'隐藏窗口';});
  document.getElementById('toggle-blur').addEventListener('click',event=>{const off=panel.classList.toggle('blur-off');event.currentTarget.setAttribute('aria-pressed',off);event.currentTarget.textContent=off?'恢复模糊':'关闭模糊';});
  document.getElementById('reset-position').addEventListener('click',()=>{panel.style.left='373px';panel.style.top='131px';});
  const handle=document.getElementById('drag-handle');
  handle.addEventListener('pointerdown',event=>{if(event.target.closest('button'))return;drag={x:event.clientX,y:event.clientY,left:panel.offsetLeft,top:panel.offsetTop};handle.setPointerCapture(event.pointerId);event.preventDefault();});
  handle.addEventListener('pointermove',event=>{if(!drag)return;panel.style.left=Math.max(0,Math.min(732,drag.left+(event.clientX-drag.x)/scale))+'px';panel.style.top=Math.max(0,Math.min(439,drag.top+(event.clientY-drag.y)/scale))+'px';});
  handle.addEventListener('pointerup',()=>{drag=null;});handle.addEventListener('lostpointercapture',()=>{drag=null;});
})();
