/** Replay of the independently approved HTML source only. No product view imports. */
import {getGeneralVisualContext, reportGeneralVisualReady} from './native-context.ts';
const longName='サクラノ詩 Re:Graphic 10th Anniversary edition — 長いゲーム名と保存された読書設定の表示確認';
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const ledger=[];
function node(selector){const value=document.querySelector(selector);if(!value)throw new Error('Approved source selector missing: '+selector);return value;}
function click(selector){node(selector).click();ledger.push({action:'click',selector});}
function game(){click('#scope-button');click('[data-scope="game"]');}
function rect(element){const r=element.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};}
async function settled(){
  await document.fonts.ready;
  await delay(150);
  await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
}
async function hiddenToast(){
  const started=performance.now();
  while(!node('#toast').hidden && performance.now()-started<4000)await delay(30);
  if(!node('#toast').hidden)throw new Error('Source finite save notification did not settle');
}
function sourceFacts(context){
  const api=window.SettingsStructureCandidate;
  const selected=document.activeElement;
  const name=element=>element.getAttribute('aria-label')||element.textContent.trim();
  return {title:document.title,headings:[...document.querySelectorAll('h1,h2')].filter(e=>e.getClientRects().length).map(e=>e.textContent.trim()),text:document.body.innerText.slice(0,8192),
    switches:[...document.querySelectorAll('[role="switch"]')].map(e=>({label:e.getAttribute('aria-labelledby')?node('#'+e.getAttribute('aria-labelledby')).textContent.trim():name(e),selected:e.getAttribute('aria-checked')==='true',disabled:e.disabled,rect:rect(e)})),
    buttons:[...document.querySelectorAll('button')].filter(e=>e.getClientRects().length).map(e=>({label:name(e),disabled:e.disabled,rect:rect(e)})),
    scope:{kind:api.state.scope,label:node('#scope-button').textContent.trim(),disabled:node('#scope-button').disabled,expanded:node('#scope-button').getAttribute('aria-expanded')==='true',raw:api.state.scope},status:node('#save-status').textContent.trim(),dirty:api.state.dirty,menuOpen:!node('#scope-options').hidden,
    focused:{tag:selected?.tagName||'',id:selected?.id||'',label:selected?.getAttribute('aria-label')||selected?.textContent?.slice(0,120)||''},
    geometry:Object.fromEntries(Object.entries({frame:'.window',sidebar:'.sidebar',sidebar_footer:'.sidebar-foot',header:'.heading',title:'h1',scroll:'.content',lexical_card:'.lexical-card',records_card:'[aria-labelledby="records-title"] .card',local_card:'.local-group>.card',footer:'.footer',scope:'#scope-button'}).map(([key,selector])=>{const e=document.querySelector(selector);return [key,e&&!e.closest('[hidden]')?rect(e):null];})),
    scrolls:[...document.querySelectorAll('.content,.sidebar')].map(e=>({class:e.className,top:e.scrollTop,height:e.clientHeight,scrollHeight:e.scrollHeight})),
    theme:document.documentElement.dataset.theme,zoom:Number(getComputedStyle(document.documentElement).zoom)||1,fontStatus:document.fonts.status,
    documentVisibility:document.visibilityState,hasFocus:document.hasFocus(),userAgent:navigator.userAgent,
    replay:ledger,errors:[],renderSource:'independent-user-approved-general-html',
    provenance:{controlledTransport:true,sourceApiKind:api.kind,nativeVisualOnly:context.transport==='packaged_native',osRuntimeVerified:false,storageRuntimeVerified:false,gameTestsExecuted:false,canvasId:context.canvasId},
    caseAssertions:{approvedSourcePresent:!!api,sourceGapState:document.body.dataset.candidateState||'normal',nativeContext:document.documentElement.dataset.context},
    loadedResources:performance.getEntriesByType('resource').map(r=>({name:r.name,initiatorType:r.initiatorType})),
  };
}
async function run(){
  const c=getGeneralVisualContext();if(c.side!=='reference')throw new Error('Reference source context required');
  if(!window.SettingsStructureCandidate||!window.GeneralStateGapCandidate)throw new Error('Approved General source did not initialize');
  if(document.documentElement.dataset.context!=='native-proposal')throw new Error('User-approved Native reference environment required');
  const suffix=c.stateId.replace('all-page-settings-general-','');
  switch(suffix){
    case 'scope-game':game();break;
    case 'restore-inheritance':game();click('#ruby-switch');click('#save');await hiddenToast();click('#restore-inheritance');break;
    case 'disabled':click('#pos-switch');click('#seg-switch');break;
    case 'long-text':{
      game();
      for(const selector of ['#scope-value','[data-scope="game"]','.status-scope'])node(selector).textContent='仅此游戏 · '+longName;
      ledger.push({action:'controlled_source_text_data',selectors:['#scope-value','[data-scope="game"]','.status-scope'],value:longName});break;
    }
    case 'keyboard-focus-escape':{
      click('#scope-button');
      const options=node('#scope-options');
      options.dispatchEvent(new KeyboardEvent('keydown',{key:'Home',bubbles:true}));
      options.dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));
      if(c.subcase!=='menu-open')options.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
      ledger.push({action:'keyboard',keys:c.subcase==='menu-open'?['Home','End']:['Home','End','Escape']});break;
    }
    case 'normal':case 'empty':case 'loading':case 'error':case 'save-failed':case 'light':case 'narrow':case 'zoom-200':case 'scope-global':break;
    default:throw new Error('Unknown source replay state');
  }
  if(suffix!=='keyboard-focus-escape' && document.activeElement instanceof HTMLElement){document.activeElement.blur();ledger.push({action:'normal_content_focus'});}
  const scroll=node('.content');
  if(c.subcase==='content-end'){scroll.scrollTop=scroll.scrollHeight;ledger.push({action:'scroll_content_end'});}
  else {scroll.scrollTop=0;ledger.push({action:'scroll_content_top'});}
  await settled();
  const facts=sourceFacts(c);
  if(facts.theme!==c.theme||facts.zoom!==c.zoom)throw new Error('Source canvas properties differ');
  await reportGeneralVisualReady(facts);
}
run().catch(async error=>{try{await reportGeneralVisualReady({errors:[String(error)],renderSource:'independent-user-approved-general-html'},String(error));}catch(reportError){console.error('General source observation failed',String(error),String(reportError));}});
