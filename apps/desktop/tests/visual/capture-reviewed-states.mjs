/** Real production-component counterparts for the independently authored v4
 * states. Only declared source data is shared, never reference pixels or CSS. */
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { historicalCaptureBlock } from './capture-catalogue.mjs';
const hash = text => createHash('sha256').update(text).digest('hex');
function unit(raw, fixture) {
  let byte=0,utf16=0;
  const tokens=(raw.tokens??[]).map(token=>{const byteStart=byte,utf16Start=utf16;byte+=Buffer.byteLength(token.surface);utf16+=token.surface.length;return{surface:token.surface,lemma:token.lemma??token.surface,reading:token.reading??'',partOfSpeech:token.pos,byteStart,byteEnd:byte,utf16Start,utf16End:utf16,known:true};});
  return {unitId:raw.id,sessionId:raw.sessionId,gameId:raw.gameId,textRevision:raw.revision??1,sourceText:raw.source,sourceHash:hash(raw.source),sourceLanguage:raw.language??fixture.game.language,createdAtMs:fixture.clock,sourceSummary:{sessionId:raw.sessionId,gameId:raw.gameId,sourceKind:'hook',state:'active',startedAtMs:fixture.clock,endedAtMs:null},tokens,translations:raw.translation?[{translationId:raw.id+':translation',taskId:raw.id+':translation-task',text:raw.translation,providerLabel:raw.provider??'Microsoft · 日中翻译',targetLanguage:'zh-CN',correction:null}]:[]};
}
async function publishSession(page,fixture,gameTemplate,frameJpeg) {
  if (fixture.current.scene !== frameJpeg.source.replace('docs/design/first-product-v1/','')) throw Error('Session preview requires the unchanged frame declared by its independent scene input');
  // This adapter supports only the independently declared completed scenario.
  // Translation presence, live queries and global task/history data are not status inputs.
  const raw=fixture.current;
  if(raw.translationStatus!=='complete')throw Error('Independent session reading fixture must explicitly declare current.translationStatus=complete');
  if(typeof raw.provider!=='string'||!raw.provider.trim()||typeof raw.translation!=='string'||!raw.translation.trim())throw Error('Independent completed reading fixture requires an explicit provider and translation');
  if(raw.expectedTranslation!==undefined&&raw.expectedTranslation!==raw.translation)throw Error('Independent completed reading fixture translation contradicts its declared expectedTranslation');
  if(typeof raw.id!=='string'||!raw.id.trim()||!Number.isSafeInteger(raw.revision)||raw.revision<=0||raw.sessionId!==fixture.session.sessionId||raw.gameId!==fixture.game.id||gameTemplate.gameId!==fixture.game.id)throw Error('Independent completed reading fixture has inconsistent unit, revision, session or game references');
  const current=unit(raw,fixture),history=fixture.history.map(raw=>unit(raw,fixture));
  const game={...structuredClone(gameTemplate),displayName:fixture.game.title,originalTitle:fixture.game.subtitle,target:{...gameTemplate.target,launchPath:fixture.session.runtimeTarget.launchPath,canonicalExePath:fixture.session.runtimeTarget.targetPath,workingDirectory:fixture.session.runtimeTarget.cwd}};
  const records=fixture.records.records.map(row=>({kind:'extraction',sequence:String(row.sequence),eventId:row.id,sourceId:row.streamId,wireSequence:String(row.sequence),text:row.source,observedAtMs:fixture.clock,reason:row.kind,bodyRefs:row.unitId?[{unitId:row.unitId,textRevision:1}]:[],payloadRetained:true}));
  const events=fixture.records.logs.map(row=>({kind:'hook_event',sequence:String(row.id),eventId:`${fixture.session.sessionId}:hook:${row.id}`,operationId:row.operationId??fixture.session.sessionId,sessionId:fixture.session.sessionId,sourceId:row.streamId??null,wireSequence:null,observedAtMs:fixture.clock,level:row.level,stage:row.phase,code:row.code??'HOOK_LIFECYCLE_EVENT',message:row.message,lostCount:null,integrity:'complete_received_range'}));
  let body=fixture.records.records.filter(row=>row.selected).map((row,index)=>{const reading=history.find(item=>item.unitId===row.unitId)??current;const start=row.source.indexOf(reading.sourceText);if(start<0)throw Error('Independent body fixture is not contained in its original extraction event');const byteStart=Buffer.byteLength(row.source.slice(0,start));return{kind:'body',sequence:String(index+1),unit:reading,sourceRefs:[{eventId:row.id,byteStart,byteEnd:byteStart+Buffer.byteLength(reading.sourceText)}]};});
  const typed=fixture.supplement;if(typed){if(typed.dataset==='body')body=typed.page.records;else if(typed.dataset==='extraction')records.splice(0,records.length,...typed.page.records);else events.splice(0,events.length,...typed.page.records);}
  const readingTarget=await page.evaluate(({current,history,game,records,events,body,fixture,frameJpeg})=>{const port=window.uiHarness,id=fixture.session.sessionId;
    // One explicit test-only lane represents the input's one completed provider.
    // It is not a SQLite head, engine membership or real attempt-selection oracle.
    /** @type {import('../../src/bridge/contracts.generated').ReadingUnitProjection} */
    const readingProjection={profileId:port.state.profileId,serverInstanceId:port.state.serverInstanceId,profileGeneration:port.state.profileGeneration,commitRevision:port.state.commitRevision,unit:current,translationState:{unitId:current.unitId,textRevision:current.textRevision,sourceHash:current.sourceHash,engines:[{scopeKey:'reviewed-visual-test-only:single-provider',bindingId:'reviewed-visual-test-only:binding',engineId:'reviewed-visual-test-only:engine',providerLabel:fixture.current.provider,status:{kind:'current',task:{taskId:current.translations[0].taskId,unitId:current.unitId,textRevision:current.textRevision,purpose:'translation',status:'completed',errorCode:null,partialText:null,providerLabel:fixture.current.provider,attemptId:null}}}]}};
    const readingTarget={profileId:readingProjection.profileId,serverInstanceId:readingProjection.serverInstanceId,profileGeneration:readingProjection.profileGeneration,commitRevision:readingProjection.commitRevision,unitId:current.unitId,textRevision:current.textRevision,sourceHash:current.sourceHash,queryStart:port.readingProjectionQueries.length};
    port.readingProjectionFixtures[JSON.stringify([current.unitId,current.textRevision])]=readingProjection;
    // Prepare the authoritative transport before publishing the new session:
    // its production WindowPreview mounts with this session's exact lease.
    const prepared=port.prepareSessionPreview(id,'b'.repeat(64),'live');
    const projection={...prepared,attemptedAtMs:String(fixture.clock),capturedAtMs:String(fixture.clock),contentChangedAtMs:String(fixture.clock),permissionCheckedAtMs:String(fixture.clock),detail:'所选窗口的当前画面。'};
    const payload=[...Uint8Array.from(atob(frameJpeg.base64),character=>character.charCodeAt(0))];
    port.emitPreview({projection,frame:{leaseId:projection.leaseId,generation:projection.generation,sequence:'1',targetIdentityHash:projection.targetIdentityHash,capturedAtMs:String(fixture.clock),width:frameJpeg.width,height:frameJpeg.height,contentHash:frameJpeg.sha256,encoding:'jpeg',payload}});
    port.bindingFixture={state:'bound',sessionId:id,targetIdentitySha256:'b'.repeat(64),windowTitle:fixture.game.title,verifiedAtMs:String(fixture.clock),errorCode:null,detail:'Explicit independent UI scenario binding; no native capture proof.'};port.sessionRecordFixtures[`${id}:body`]=body;port.sessionRecordFixtures[`${id}:extraction`]=records;port.sessionRecordFixtures[`${id}:hook_events`]=events;if(fixture.supplement)port.sessionCompletenessFixtures[`${id}:${fixture.supplement.dataset}`]=fixture.supplement.page.completeness;port.publish({games:[game],current,history:{units:history,nextCursor:null},session:{sessionId:id,gameId:game.gameId,sourceKind:'hook',state:'active',paused:false,translationEnabled:true,sources:fixture.game.streams.map(stream=>({sourceId:stream.id,hookName:stream.label,hookCode:stream.hook,encoding:'UTF-16',sample:stream.preview,selected:fixture.session.selectedStreams.includes(stream.id)}))}});return readingTarget;},{current,history,game,records,events,body,fixture,frameJpeg});
  if (await page.getByRole('region',{name:'运行窗口预览',exact:true}).count()) await verifyPreviewFrame(page,frameJpeg);
  // Call only after the existing body journey activates C17; inactive library
  // and extraction/Hook tabs must not be activated just to manufacture a query.
  return async()=>{
    await page.waitForFunction(target=>{const port=window.uiHarness;if(port.state.profileId!==target.profileId||port.state.serverInstanceId!==target.serverInstanceId||port.state.profileGeneration!==target.profileGeneration||port.state.current?.unitId!==target.unitId||port.state.current?.textRevision!==target.textRevision||port.state.current?.sourceHash!==target.sourceHash)throw Error('Reviewed body reading owner changed before capture');const owner=JSON.stringify([target.profileId,target.serverInstanceId,target.profileGeneration,target.unitId,target.textRevision]);return port.readingProjectionQueries.slice(target.queryStart).some(query=>query.unitId===target.unitId&&query.textRevision===target.textRevision&&query.commitRevision>=target.commitRevision)&&port.readingProjectionActive.get(owner)===0;},readingTarget,{timeout:10000});
    await page.getByRole('article',{name:'当前原文与译文',exact:true}).getByText('已完成',{exact:true}).waitFor({state:'visible',timeout:10000});
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  };
}
// Executed in the browser against the real test-port subscription map. The
// caller carries all three owner fields; unrelated/stale Map insertions cannot
// select a frame recipient or silently hide duplicate active owners.
export function assertPreviewOwner(owner) {
  const port = window.uiHarness;
  if (!owner?.consumerId || !owner.leaseId || !owner.generation) throw Error('Preview injection requires explicit consumerId, leaseId and generation');
  const projection = port.previewProjections.get(owner.leaseId);
  const same = value => value.consumerId === owner.consumerId && value.leaseId === owner.leaseId && value.generation === owner.generation;
  const active = [...port.previewProjections.values()].filter(value => value.consumerId === owner.consumerId && value.state !== 'stopped');
  const subscriptions = [...port.previewListeners.values()].filter(value => value.projection.consumerId === owner.consumerId);
  if (!projection || !same(projection) || active.length !== 1 || !same(active[0]) || subscriptions.length !== 1 || !same(subscriptions[0].projection)) throw Error('Preview injection owner must have one exact active consumer/lease/generation and subscription');
  return structuredClone(projection);
}

async function previewOwner(page, { consumerId, candidateId, leaseId }) {
  const owner = await page.evaluate(({consumerId,candidateId,leaseId}) => {
    const port = window.uiHarness;
    if (candidateId) {
      const requests = port.previewRequests.filter(request => request.candidateId === candidateId);
      if (requests.length !== 1) throw Error('Target preview requires one exact candidate owner request');
      consumerId = requests[0].consumerId;
    }
    const projections = [...port.previewProjections.values()].filter(value => value.consumerId === consumerId && value.state !== 'stopped' && (!leaseId || value.leaseId === leaseId));
    if (projections.length !== 1) throw Error('Preview owner projection is missing or ambiguous');
    const projection = projections[0];
    return { consumerId, leaseId: projection.leaseId, generation: projection.generation };
  }, {consumerId,candidateId,leaseId});
  await page.waitForFunction(owner => [...window.uiHarness.previewListeners.values()].some(({projection}) => projection.consumerId === owner.consumerId && projection.leaseId === owner.leaseId && projection.generation === owner.generation), owner, {timeout:10000});
  await page.evaluate(assertPreviewOwner, owner);
  return owner;
}

async function emitPreview(page,{owner,state='live',clock,frameJpeg,detail}) {
  const previous = await page.evaluate(assertPreviewOwner, owner);
  await page.evaluate(async({previous,state,clock,frameJpeg,detail})=>{const port=window.uiHarness;const active=[...port.previewProjections.values()].filter(value=>value.consumerId===previous.consumerId&&value.state!=='stopped'),subscribers=[...port.previewListeners.values()].filter(value=>value.projection.consumerId===previous.consumerId);if(active.length!==1||subscribers.length!==1||[active[0],subscribers[0].projection].some(value=>value.leaseId!==previous.leaseId||value.generation!==previous.generation))throw Error('Preview injection owner changed before emit');const projection={...previous,state,detail:detail??(state==='live'?'所选窗口的当前画面。':state==='paused'?'预览已暂停，保留最后成功画面。':state==='permission_denied'?'需要屏幕录制权限，取文接入仍可继续。':state==='target_closed'?'目标窗口已关闭，保留最后成功画面。':'当前运行环境无法提供可核验的窗口画面。'),permission:state==='permission_denied'?'denied':'granted',capturedAtMs:String(clock)};let frame=null;if(frameJpeg){const payload=[...Uint8Array.from(atob(frameJpeg.base64),character=>character.charCodeAt(0))];frame={leaseId:projection.leaseId,generation:projection.generation,sequence:'1',targetIdentityHash:projection.targetIdentityHash,capturedAtMs:String(clock),width:frameJpeg.width,height:frameJpeg.height,contentHash:frameJpeg.sha256,encoding:'jpeg',payload};}port.emitPreview({projection,frame});},{previous,state,clock,frameJpeg,detail});
  if(frameJpeg)await verifyPreviewFrame(page,frameJpeg);
}
async function verifyPreviewFrame(page,frameJpeg) {
  {const image=page.getByRole('region',{name:'运行窗口预览',exact:true}).locator('img');await image.waitFor();const decoded=await image.evaluate(async element=>{await element.decode();return{complete:element.complete,width:element.naturalWidth,height:element.naturalHeight,source:element.currentSrc};});if(!decoded.complete||decoded.width!==frameJpeg.width||decoded.height!==frameJpeg.height||decoded.width<=0||decoded.height<=0||!decoded.source.startsWith('blob:'))throw Error('Preview JPEG must decode at its unchanged fixture dimensions under production CSP');}
}
/** Install exact controlled transport data before the production preview mounts.
 * The bootstrap's existing test-only initializer performs no native capture. */
async function reviewedPreviewPage(pageFor,viewport,inputs,frameJpeg) {
  // The initial empty window provides the existing browser/CSP setup. Only the
  // second navigation mounts the component, after the explicit initializer.
  const page=await pageFor('main','empty',viewport);
  await page.addInitScript(({clock,frameJpeg})=>{
    window.__initializeControlledPort=port=>{
      const prepared=port.prepareSessionPreview('controlled-approved-preview-consumer','f'.repeat(64),'live');
      const projection={...prepared,attemptedAtMs:String(clock),capturedAtMs:String(clock),contentChangedAtMs:String(clock),permissionCheckedAtMs:String(clock),detail:'所选窗口的当前画面。'};
      const payload=[...Uint8Array.from(atob(frameJpeg.base64),character=>character.charCodeAt(0))];
      port.emitPreview({projection,frame:{leaseId:projection.leaseId,generation:projection.generation,sequence:'1',targetIdentityHash:projection.targetIdentityHash,capturedAtMs:String(clock),width:frameJpeg.width,height:frameJpeg.height,contentHash:frameJpeg.sha256,encoding:'jpeg',payload}});
      port.reviewedCandidatePreviewFixture={candidateId:'controlled-approved-preview-target',projection};
    };
  },{clock:inputs.clock,frameJpeg});
  await page.goto(page.url()+'&reviewedSurface=preview');
  await page.getByRole('region',{name:'运行窗口预览',exact:true}).waitFor();
  await verifyPreviewFrame(page,frameJpeg);
  return page;
}
export async function captureReviewedStates({catalogue,pageFor,screenshot,publishDesignReading,designLibraryGame,designTarget,selectValue,root,check,captureOnly}) {
  if(!catalogue||catalogue.schemaVersion!==4)return;
  const states=catalogue.states.filter(state=>state.requirements?.length);
  const sessionDefinition=catalogue.states.find(state=>state.id==='session-current-extraction');
  const sessionFixture=JSON.parse(await fs.readFile(catalogue.resolveReference(sessionDefinition.dataState.fixtureFile)));
  const frameDefinition=catalogue.states.find(state=>state.id==='preview-live').dataState.componentInputs.frameInput;const frameBytes=await fs.readFile(catalogue.resolveReference('docs/design/first-product-v1/acceptance-v4/'+frameDefinition.file));if(hash(frameBytes)!==frameDefinition.sha256)throw Error('Independent JPEG fixture identity changed');const frameJpeg={...frameDefinition,base64:frameBytes.toString('base64')};
  for(const state of states) {
    if(captureOnly&&!captureOnly.has(state.id))continue;
    await check(`V4-${state.id}-actual-entry-and-conditions`,async()=>{
      const first=state.variants[0];let page;
      try {
        const blocked = historicalCaptureBlock(catalogue,state.id);
        if(blocked) { await screenshot(null,state.id,null); throw Error(`blocked_pending_replacement_reference: ${blocked.code}: ${blocked.reason}`); }
        if(state.surfaceKind==='component') {
          const inputs=state.dataState.componentInputs,kind=state.id.startsWith('hud-')?'hud':state.id.startsWith('select-')?'select':state.id.startsWith('preview-')?'preview':state.id;
          page=kind==='preview'?await reviewedPreviewPage(pageFor,first.viewport,inputs,frameJpeg):await pageFor(kind==='hud'?'hud':'main','reading',first.viewport,kind==='hud'?'':`&reviewedSurface=${kind}${state.id==='select-long-list'?'&longList=true':''}`);
          if(kind==='hud'){
            await publishDesignReading(page);
            await page.evaluate(({id,inputs})=>{const p=window.uiHarness,hud=inputs.hud,preferences={...p.state.preferences,...(hud?{segmentationEnabled:hud.segmentation,showRuby:hud.ruby,showPos:hud.pos,hudBackgroundOpacity:hud.hudBackgroundOpacity}:{}),...(id==='hud-toolbar-500'?{overrides:{...p.state.preferences.overrides,'hud.compact':true},hudWidthPt:500}:{})};const current=p.state.current;p.publish({preferences,...(id==='hud-lexical-error'?{lexicalStates:[{unitId:current.unitId,textRevision:current.textRevision,sourceHash:current.sourceHash,state:'worker_failed',errorCode:'controlled_worker_failed'}]}:{})});},{id:state.id,inputs});
          } else if(kind==='select') await page.getByRole('combobox',{name:'组件示例文本来源',exact:true}).click();
          else if(kind==='preview') {
            const desired={'preview-paused':'paused','preview-permission':'permission_denied','preview-unavailable':'unavailable','preview-exited':'target_closed'}[state.id];
            if(state.id==='preview-paused'){
              await page.getByRole('button',{name:'暂停',exact:true}).click();
              await page.getByRole('button',{name:'继续',exact:true}).waitFor();
            }
            if(desired)await emitPreview(page,{owner:await previewOwner(page,{consumerId:'controlled-approved-preview-consumer'}),clock:inputs.clock,state:desired,detail:inputs.previewDescription});
            if(await page.getByRole('region',{name:'运行窗口预览',exact:true}).getAttribute('data-preview-state')!==(desired??'live'))throw Error('Actual production preview did not reach the declared controlled transport state');
          } else {
            await publishSession(page,{...sessionFixture,supplement:inputs.supplement},designLibraryGame,frameJpeg);
            await page.getByRole('log').waitFor();
            // The independent Hook fixture requests all events. Reach it via
            // the visible production Select; diagnostics remains the default.
            const expected = inputs.supplement?.query;
            if(expected?.dataset==='hook_events') await selectValue(page.getByRole('combobox',{name:'事件范围',exact:true}),expected.filter.stage??'');
            if(expected) await page.waitForFunction(expected=>{const query=window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId===expected.sessionId&&query.dataset===expected.dataset).at(-1);return query&&JSON.stringify([query.sessionId,query.dataset,query.filter.search,query.filter.sourceId,query.filter.level,query.filter.stage,query.order,query.pageSize,query.cursor])===JSON.stringify([expected.sessionId,expected.dataset,expected.filter.search,expected.filter.sourceId,expected.filter.level,expected.filter.stage,expected.order,expected.pageSize,expected.cursor]);},expected,{timeout:10000});
            await page.waitForFunction(()=>document.querySelector('[data-record-sequence]'));
          }
        } else if(state.id.startsWith('session-current-')) {
          const fixture=JSON.parse(await fs.readFile(catalogue.resolveReference(state.dataState.fixtureFile)));page=await pageFor('main','reading',first.viewport);const waitForReadingProjection=await publishSession(page,fixture,designLibraryGame,frameJpeg);await page.getByRole('button',{name:'当前会话',exact:true}).click();const tab=state.id.endsWith('body')?'正文历史':state.id.endsWith('extraction')?'全部提取':'Hook 日志';await page.getByRole('tab',{name:tab,exact:true}).click();await page.waitForFunction(()=>document.querySelector('[data-record-sequence]'));if(tab==='正文历史')await waitForReadingProjection();
        } else if(state.id==='library-populated') {
          page=await pageFor('main','empty',first.viewport);await page.evaluate(game=>window.uiHarness.publish({games:[game]}),designLibraryGame);
        } else if(state.id.startsWith('target-review-preview-')) {
          page=await pageFor('main','empty',first.viewport);await page.evaluate(({game,target})=>{const port=window.uiHarness;port.targetCandidates=[target];port.publish({games:[game]});},{game:designLibraryGame,target:designTarget});await page.getByRole('button',{name:'连接运行中的游戏…',exact:true}).first().click();await page.getByRole('button',{name:'点击选择窗口…',exact:true}).click();await page.getByRole('region',{name:'关联与实际取文目标',exact:true}).waitFor();if(await page.getByRole('button',{name:'核对目标',exact:true}).count())throw Error('Target review must retain the current single-confirmation entry');if(await page.evaluate(()=>window.uiHarness.associations.length)!==0)throw Error('Target preview must not associate or inject Hook before confirmation');await emitPreview(page,{owner:await previewOwner(page,{candidateId:designTarget.candidateId}),clock:sessionFixture.clock,frameJpeg});
        } else {
          const fixture=JSON.parse(await fs.readFile(catalogue.resolveReference(state.dataState.fixtureFile))),typed=fixture.supplement;if(!typed)throw Error('Scoped preferences require independent typed supplement input');
          const active=JSON.parse(await fs.readFile(catalogue.resolveReference('docs/design/first-product-v1/acceptance-v4/reviewed-supplement/settings-shortcuts-active.input.json')));
          page=await pageFor('settings','empty',first.viewport);await page.evaluate(({typed,active})=>{const port=window.uiHarness;port.capabilityFixture=structuredClone(active.capabilities);port.scopedPreferenceSaves.set('global',{saved:structuredClone(typed.preferences),headRevision:4,preferenceRevisionId:'independent-design-global-4'});port.publish({preferences:typed.preferences});},{typed,active});await page.getByRole('button',{name:'显示与快捷键',exact:true}).click();await page.waitForFunction(()=>[...document.querySelectorAll('input[type=number]')].some(node=>node.value==='24'));
          if(state.id.endsWith('conflict')){const pause=page.getByRole('textbox',{name:'暂停 / 继续取文快捷键',exact:true});await pause.fill(typed.draft.shortcuts.togglePause);await page.evaluate(typed=>window.uiHarness.scopedPreferenceSaveResponses.push({error:typed.saveError,capabilities:typed.capabilities}),typed);await page.getByRole('button',{name:'应用快捷键',exact:true}).click();await page.getByRole('alert').filter({hasText:typed.saveError}).waitFor();await pause.focus();await pause.blur();await page.getByText('应用失败，原生效组合保留',{exact:true}).waitFor();}

        }
        await screenshot(page,state.id,null);
      } finally {await page?.close();}
    });
  }
}
