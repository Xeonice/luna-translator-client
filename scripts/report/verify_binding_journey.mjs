/** Actual production MainWindow/CaptureSheet journeys, with explicitly controlled transport.
 * Native/SQLite/real Hook acceptance are separate. Missing product behavior is a
 * semantic failed assertion, never an expected-failure success.
 */
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { launchLockedBrowser, verifyBrowserIdentity } from '../../apps/desktop/tests/visual/locked-browser.mjs';

// Finite code-owned UI scenarios. A producer advertises inputs, never verdicts.
export const ORIGINAL_COMPONENT_SCENES = [
  ['01', 'library-window'], ['01', 'library-process'],
  ['02', 'first-confirmation'], ['02', 'tcc'], ['03', 'pairing-transfer'],
  ['04', 'idle-refresh-details'], ['04', 'picker-cancel'], ['04', 'picker-timeout'],
  ['05', 'reuse'], ['05', 'switch-cancel'], ['05', 'stale-review'],
  ['06', 'real-stage-order'], ['06', 'stale-events'],
  ['07', 'cancel-candidate'], ['07', 'close-operation'],
  ['08', 'empty'], ['08', 'draft-later'], ['08', 'commit-set'], ['08', 'commit-failure'],
  ['09', 'owned-remember'], ['09', 'unowned-remember'], ['09', 'temporary-saved'], ['09', 'requires-review'],
  ['10', 'details'], ['13', 'effects-order'], ['13', 'effects-retry'], ['13', 'stale-effects'],
  ['14', 'layout-normal'], ['14', 'layout-narrow'], ['14', 'layout-200'], ['14', 'other-bottle-warning'],
  ['15', 'frame-state'], ['15', 'revoke'], ['15', 'late-metadata'],
  ['17', 'association-retry'], ['17', 'source-query-retry'], ['17', 'source-expired'], ['17', 'submit-retry'],
].map(([journey, name]) => ({ id: `BJS-${journey}-${name}`, journey_id: `BJ-${journey}`, name }));
export const COMPONENT_SCENES = [...ORIGINAL_COMPONENT_SCENES,
  ...['library-global-owner','scoped-reconnect-owner'].map(name=>({id:`BJS-01-${name}`,journey_id:'BJ-01',name}))];
const RPC_PREFIX = 'LUNA_BINDING_RPC_V1 ';
const RPC_SCHEMA = 1;
function strictProtocolJson(raw) {
  const value = JSON.parse(raw), stack = [];
  const tokens = raw.match(/"(?:[^"\\]|\\.)*"|[{}\[\]:,]|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null/g) ?? [];
  for (const token of tokens) {
    if (token === '{') stack.push({ object: true, keys: new Set(), expectingKey: true });
    else if (token === '[') stack.push({ object: false });
    else if (token === '}' || token === ']') stack.pop();
    else if (token === ',') { if (stack.at(-1)?.object) stack.at(-1).expectingKey = true; }
    else if (token === ':') { if (stack.at(-1)?.object) stack.at(-1).expectingKey = false; }
    else if (token.startsWith('"') && stack.at(-1)?.object && stack.at(-1).expectingKey) {
      const key = JSON.parse(token), current = stack.at(-1);
      assert.ok(!current.keys.has(key), 'duplicate RPC JSON object key'); current.keys.add(key);
    }
  }
  return value;
}
export class ProductionRpc {
  constructor({ binary, nonce, sourceTree, cwd, output }) {
    Object.assign(this, { binary, nonce, sourceTree, cwd, output });
    this.stdout = ''; this.stderr = ''; this.stdin = ''; this.buffer = '';
    this.nextId = 1n; this.pending = new Map(); this.transcript = [];
    this.ready = null; this.protocolErrors = []; this.startedAt = null; this.finishedAt = null;
    this.exitCode = null; this.exitSignal = null; this.closed = false;
  }
  fail(error) {
    const message = String(error.message ?? error); this.protocolErrors.push(message);
    for (const row of this.pending.values()) { clearTimeout(row.timer); row.reject(Error(message)); }
    this.pending.clear(); this.child?.kill('SIGTERM');
  }
  line(line) {
    // Serial libtest writes its exact test-name prefix before the first stdout
    // record. Strip only that known harness prefix, never arbitrary log text.
    const harness = 'test bj_component_transport ... ';
    if (line.startsWith(harness + RPC_PREFIX)) line = line.slice(harness.length);
    if (!line.startsWith(RPC_PREFIX)) return; // Rust test-harness output stays in stdout.
    let message;
    try {
      message = strictProtocolJson(line.slice(RPC_PREFIX.length));
      assert.ok(message && typeof message === 'object' && !Array.isArray(message));
      assert.equal(message.schema_version, RPC_SCHEMA); assert.equal(message.nonce, this.nonce);
      if (message.type === 'ready') {
        assert.equal(this.ready, null, 'duplicate RPC ready');
        assert.equal(message.source_tree_sha256, this.sourceTree);
        assert.deepEqual(Object.keys(message).sort(), ['capabilities', 'nonce', 'schema_version', 'source_tree_sha256', 'type']);
        const capability = message.capabilities;
        assert.ok(capability && Array.isArray(capability.scene_ids) && Array.isArray(capability.methods) && Array.isArray(capability.controls));
        assert.deepEqual(Object.keys(capability).sort(), ['controls', 'methods', 'scene_ids']);
        for (const field of ['scene_ids', 'methods', 'controls']) {
          assert.ok(capability[field].every(value => typeof value === 'string' && value));
          assert.equal(new Set(capability[field]).size, capability[field].length);
        }
        assert.ok(capability.scene_ids.every(id => COMPONENT_SCENES.some(row => row.id === id)), 'unregistered component scene');
        assert.ok(capability.controls.every(value => ['reset', 'deliver'].includes(value)));
        assert.ok(capability.methods.every(value => /^[A-Za-z][A-Za-z0-9]*$/.test(value) && !['constructor','prototype','then'].includes(value)), 'invalid RPC method capability');
        this.ready = message; this.readyResolve?.(message); return;
      }
      assert.equal(message.type, 'response');
      assert.match(message.id, /^(0|[1-9]\d*)$/);
      const pending = this.pending.get(message.id); assert.ok(pending, 'unknown or repeated RPC response ID');
      const request = pending.request;
      for (const field of ['scenario', 'action', 'method']) assert.equal(message[field], request[field], 'RPC correlation differs: ' + field);
      assert.ok(['ok', 'error'].includes(message.outcome));
      assert.deepEqual(Object.keys(message).sort(), ['action', 'error', 'events', 'id', 'method', 'nonce', 'outcome', 'previewUpdates', 'publications', 'response', 'scenario', 'schema_version', 'type']);
      for (const field of ['publications', 'previewUpdates', 'events']) assert.ok(Array.isArray(message[field]));
      assert.ok(message.outcome === 'ok' ? message.error === null : message.response === null);
      clearTimeout(pending.timer); this.pending.delete(message.id);
      this.transcript.push({ request, response: message }); pending.resolve(message);
    } catch (error) { this.fail(error); }
  }
  async start() {
    this.startedAt = new Date().toISOString();
    this.command = [this.binary, '--ignored', '--exact', 'bj_component_transport', '--nocapture'];
    this.environment = { LUNA_BINDING_NONCE: this.nonce, LUNA_BINDING_SOURCE_TREE: this.sourceTree, LUNA_BINDING_MODE: 'stdio' };
    if(this.output)this.environment.LUNA_BINDING_OUTPUT=this.output;
    this.child = spawn(this.command[0], this.command.slice(1), { cwd: this.cwd, env: { ...process.env, ...this.environment }, stdio: ['pipe', 'pipe', 'pipe'] });
    this.child.stdin.on('error', error => this.fail(error));
    this.child.stdout.setEncoding('utf8'); this.child.stderr.setEncoding('utf8');
    this.child.stdout.on('data', data => {
      const value = data.toString('utf8'); this.stdout += value; this.buffer += value;
      if (this.stdout.length > 64 * 1024 * 1024 || this.buffer.length > 16 * 1024 * 1024) { this.fail(Error('RPC stdout bound exceeded')); return; }
      let index; while ((index = this.buffer.indexOf('\n')) >= 0) { const line = this.buffer.slice(0, index).replace(/\r$/, ''); this.buffer = this.buffer.slice(index + 1); this.line(line); }
    });
    this.child.stderr.on('data', data => { this.stderr += data; if (this.stderr.length > 8 * 1024 * 1024) this.fail(Error('RPC stderr bound exceeded')); });
    this.child.on('error', error => { this.stderr += String(error); this.readyResolve?.(null); });
    this.exitPromise = new Promise(resolve => this.child.on('close', (code, signal) => { this.exitCode = code; this.exitSignal = signal; this.finishedAt = new Date().toISOString(); this.closed = true; if (this.buffer) this.line(this.buffer); this.readyResolve?.(null); for (const row of this.pending.values()) { clearTimeout(row.timer); row.reject(Error('RPC producer exited before response')); } this.pending.clear(); resolve(); }));
    return Promise.race([new Promise(resolve => { this.readyResolve = resolve; if (this.ready) resolve(this.ready); }), new Promise(resolve => setTimeout(() => resolve(null), 5000))]);
  }
  async request(scenario, action, method, args) {
    assert.ok(this.ready && !this.closed && !this.protocolErrors.length, 'RPC producer unavailable');
    assert.ok(this.ready.capabilities.scene_ids.includes(scenario), 'RPC scene capability missing');
    assert.ok(Array.isArray(args));
    assert.ok(action === 'invoke' ? this.ready.capabilities.methods.includes(method) : this.ready.capabilities.controls.includes(action));
    const request = { type: 'request', schema_version: RPC_SCHEMA, nonce: this.nonce, id: String(this.nextId++), scenario, action, method, args };
    const line = RPC_PREFIX + JSON.stringify(request) + '\n'; this.stdin += line;
    const promise = new Promise((resolve, reject) => { const timer = setTimeout(() => { this.pending.delete(request.id); const error = Error('RPC response timed out: ' + request.id); reject(error); this.fail(error); }, 10000); this.pending.set(request.id, { request, resolve, reject, timer }); });
    this.child.stdin.write(line); return promise;
  }
  async close() {
    if (!this.child) return;
    if (!this.closed) { this.child.stdin.end(); await Promise.race([this.exitPromise, new Promise(resolve => setTimeout(resolve, 5000))]); }
    if (!this.closed) { this.child.kill('SIGTERM'); await this.exitPromise; }
  }
}

export function captureLiveDocument() {
    const original = document.documentElement, clone = original.cloneNode(true), images = [];
    const live = [original, ...original.querySelectorAll('*')], copied = [clone, ...clone.querySelectorAll('*')];
    for (let index = 0; index < live.length; index++) {
      const source = live[index], target = copied[index];
      if (source instanceof HTMLInputElement) {
        target.toggleAttribute('checked', source.checked);
        target.toggleAttribute('disabled', source.matches(':disabled'));
        if (source.type !== 'password') target.setAttribute('value', source.value);
      } else if (source instanceof HTMLOptionElement) target.toggleAttribute('selected', source.selected);
      else if ('disabled' in source) target.toggleAttribute('disabled', source.matches(':disabled'));
      if (source.scrollTop !== 0) target.setAttribute('data-binding-observed-scroll-top', String(source.scrollTop));
      if (source.scrollLeft !== 0) target.setAttribute('data-binding-observed-scroll-left', String(source.scrollLeft));
      if (source instanceof HTMLImageElement && source.complete && source.naturalWidth > 0 && source.naturalHeight > 0) {
        const canvas = document.createElement('canvas');
        canvas.width = source.naturalWidth; canvas.height = source.naturalHeight;
        const context = canvas.getContext('2d'); if (!context) throw Error('decoded image evidence requires a 2D canvas');
        context.drawImage(source, 0, 0);
        const dataUrl = canvas.toDataURL('image/png'), imageIndex = images.length;
        const originalSource = source.getAttribute('src') ?? '';
        images.push({ originalSource, width: source.naturalWidth, height: source.naturalHeight, dataUrl });
        target.setAttribute('src', dataUrl); target.setAttribute('data-binding-original-source', originalSource);
        target.setAttribute('data-binding-decoded-image-index', String(imageIndex));
      }
    }
    return { html: '<!doctype html>\n' + clone.outerHTML, images };
}

const assertion = (id, expected, actual) => ({ id, expected, actual, passed: JSON.stringify(expected) === JSON.stringify(actual) });
const callsOf = (step, names) => step.facts.methodCalls.filter(row => names.includes(row.method));
const commitsOf = step => step.facts.methodCalls.filter(row => row.method === 'beginReading' || row.method === 'dispatch' && ['select_source','select_sources','begin_reading'].includes(row.args[0]?.command?.kind));
const sourceChoices = step => step.dom.checkboxes.filter(row => !row.label.includes('记住'));
const savedEffects = step => step.facts.events.filter(row => row.kind === 'production_side_effect').map(row => row.event);
export function sourcePanelEvidence(step) {
  if (!step) return false;
  const snapshot = step.facts.snapshot, session = snapshot?.session;
  const replies = step.facts.rpcMessages ?? [];
  const origin = replies.filter(row => row.action === 'invoke' && row.method === 'sourceSelection' && row.outcome === 'ok').at(-1);
  const projection = origin?.response;
  if (!session?.sessionId || projection?.sessionId !== session.sessionId
      || ['profileId','serverInstanceId'].some(key=>typeof projection.profile?.[key]!=='string'||!projection.profile[key])
      || !Number.isInteger(projection.profile?.profileGeneration) || projection.profile.profileGeneration < 0
      || projection.profile?.profileId !== snapshot.profileId
      || projection.profile?.serverInstanceId !== snapshot.serverInstanceId
      || projection.profile?.profileGeneration !== snapshot.profileGeneration
      || !Array.isArray(projection.sources) || !projection.sources.length) return false;
  const ids = projection.sources.map(row => row.sourceId), rows = step.dom.sourceRows ?? [], boxes = sourceChoices(step);
  if (ids.some(id => typeof id !== 'string' || !id) || new Set(ids).size !== ids.length
      || rows.length !== ids.length || boxes.length !== ids.length
      || new Set(rows.map(row => row.sourceId)).size !== ids.length
      || new Set(boxes.map(row => row.sourceId)).size !== ids.length) return false;
  return ids.every(id => rows.filter(row => row.sourceId === id && row.checkboxCount === 1 && row.visibleCheckboxCount === 1 && row.enabledCheckboxCount === 1).length === 1
    && boxes.filter(row => row.sourceId === id && !row.disabled).length === 1)
    && ['开始阅读', '稍后选择'].every(label => step.dom.buttons.filter(row => row.label === label).length === 1);
}
export function candidateReadyEvidence(step) {
  if (!sourcePanelEvidence(step)) return false;
  const receipt = step.facts.rpcMessages.filter(row => row.action === 'invoke' && row.method === 'associate' && row.outcome === 'ok').at(-1)?.response;
  const sid = step.facts.snapshot.session.sessionId;
  return typeof receipt?.operationId === 'string' && !!receipt.operationId && receipt.sessionId === sid
    && savedEffects(step).some(row => row.kind === 'association_stage' && row.operation_id === receipt.operationId && row.stage === 'candidates_ready')
    && commitsOf(step).length === 0;
}
export function scopedSourceDischarge(caseRecord,scene,nonce){
  const scopes=['advanced-source-typed-DTO-disclosure','source-u64-typed-DTO-projection'];
  const details=scene?.observations?.steps?.at(-1)?.source_details;
  if(caseRecord?.id!=='BJC-05-source-details'||scene?.id!=='BJS-10-details'||caseRecord.status!=='blocked'
    ||JSON.stringify(caseRecord.observations?.blocked_obligations)!==JSON.stringify(scopes)||scene.status!=='passed'
    ||scene.observations.transport_mode!=='production_rpc'||JSON.stringify(details?.blocked_obligations)!=='[]'
    ||details?.advancedAvailable!==true||details?.precisionAvailable!==true||!(/^[a-f0-9]{32}$/).test(nonce))return null;
  return {kind:'same-run-source-detail-discharge/v1',nonce,case_id:caseRecord.id,case_trace:caseRecord.trace,
    scene_id:scene.id,scene_trace:scene.trace,obligations:scopes};
}
export function actualProgressReadReady({facts,deliveryId}){
  facts??=window.bindingHarness.facts();
  const messages=facts.rpcMessages,delivery=messages.find(row=>row.id===deliveryId&&row.action==='deliver');
  const operation=messages.filter(row=>row.method==='associate'&&row.outcome==='ok'&&BigInt(row.id)<BigInt(deliveryId)).at(-1)?.response?.operationId;
  if(!operation||!delivery?.events.some(row=>row.kind==='association_stage'&&row.operation_id===operation))return true;
  for(const response of messages.filter(row=>row.method==='associationProgress'&&BigInt(row.id)>BigInt(deliveryId))){
    const eventIndex=facts.events.findIndex(row=>row.kind==='runtime_transport_call'&&row.method==='associationProgress'&&row.responseId===response.id&&row.args?.[0]?.operationId===operation);
    if(eventIndex<0)continue;
    const call=facts.events.slice(0,eventIndex).filter(row=>row.kind==='port_call'&&row.method==='associationProgress'&&row.args?.[0]?.operationId===operation).at(-1);
    if(call&&facts.events.slice(eventIndex+1).some(row=>['port_return','port_error'].includes(row.kind)&&row.method==='associationProgress'&&row.id===call.id))return true;
  }
  return false;
}
export function libraryOwnerChecks(name, steps) {
  const first=steps[0],last=steps.at(-1),replies=last.facts.rpcMessages??[];
  const reset=replies.find(row=>row.action==='reset'&&row.outcome==='ok')?.response??{};
  const initial=reset.initial_snapshot??{},games=initial.games??[],seeds=reset.seed_commands??[];
  const key=target=>JSON.stringify(['environmentKey','canonicalExePath','architecture'].map(field=>target?.[field]??null));
  const keys=games.map(game=>key(game.target)),seededIds=[];
  let validSeed=games.length>=2&&new Set(keys).size===keys.length&&games.every(game=>['environmentKey','canonicalExePath','architecture'].every(field=>typeof game.target?.[field]==='string'&&game.target[field]));
  for(const row of seeds){const command=row.request?.command??{},results=row.response?.gameResults,items=command.payload?.items;
    validSeed=validSeed&&command.kind==='save_games'&&Array.isArray(items)&&Array.isArray(results)&&items.length===results.length;
    if(!Array.isArray(items)||!Array.isArray(results))continue;
    for(const item of items){const matches=results.filter(value=>value.itemId===item.itemId&&Number.isInteger(value.rowRevision)&&value.rowRevision>0&&typeof value.reused==='boolean');
      const actual=games.find(game=>key(game.target)===key(item.game?.target));
      validSeed=validSeed&&matches.length===1&&!!actual&&matches[0].gameId===actual.gameId;if(matches.length===1)seededIds.push(matches[0].gameId);}}
  validSeed=!!validSeed&&new Set(seededIds).size===games.length&&games.every(game=>seededIds.includes(game.gameId));
  const review=replies.filter(row=>row.action==='invoke'&&row.method==='reviewAssociation'&&row.outcome==='ok').at(-1)?.response??{},candidate=review.candidate??{};
  const owners=games.filter(game=>key(game.target)===JSON.stringify([candidate.environmentKey??null,candidate.executablePath??null,candidate.architecture??null]));
  const owner=owners.length===1?owners[0].gameId:null;
  const selected=games.filter(game=>first.dom.buttons.some(button=>button.pressed==='true'&&game.displayName&&button.label.includes(game.displayName)));
  const selectedId=selected.length===1?selected[0].gameId:null,calls=last.facts.methodCalls??[];
  const query=calls.filter(row=>row.method==='reviewAssociation').at(-1)?.args?.[0]??{};
  const saves=calls.filter(row=>row.method==='dispatch'&&['save_game','save_games'].includes(row.args?.[0]?.command?.kind));
  const associates=calls.filter(row=>row.method==='associate'),scope=name==='scoped-reconnect-owner'?(initial.session?.gameId??null):null;
  const incompatible=name==='scoped-reconnect-owner'&&owner!==scope;
  const identityValid=typeof candidate.candidateId==='string'&&candidate.candidateId===query.candidateId&&Number.isInteger(candidate.windowsPid)&&candidate.windowsPid>0&&typeof candidate.processCreatedU64==='string'&&/^[1-9][0-9]*$/.test(candidate.processCreatedU64)&&typeof candidate.targetIdentitySha256==='string'&&/^[a-f0-9]{64}$/.test(candidate.targetIdentitySha256);
  const incompatibleStep=steps.find(step=>step.action==='scoped-incompatible');
  return [['actual-two-game-SaveGames-seed',true,validSeed],['actual-library-A-selected-before-intent',true,selectedId!==null&&selectedId!==owner],
    ['actual-full-runtime-target-identity',true,!!owner&&!!identityValid],['entry-scope-is-explicit',scope,query.gameId??null],
    ['review-retains-correct-game-owner',name==='scoped-reconnect-owner'?scope:owner,review.gameId??null],
    ['no-implicit-game-save-after-seed',0,saves.length],['exact-association-count-for-scope',incompatible?0:1,associates.length],
    ['incompatible-scope-remains-visible',true,!incompatible||!!incompatibleStep?.dom.dialogs?.length]];
}
export function actualSceneInputs(steps) {
  const messages = steps.at(-1)?.facts.rpcMessages ?? [];
  const reset = messages.find(message => message.action === 'reset' && message.outcome === 'ok');
  const protectedSessionId = reset?.response?.initial_snapshot?.session?.sessionId ?? (steps[0]?.facts.transportMode === 'controlled_diagnostic' ? steps[0]?.facts.snapshot?.session?.sessionId ?? null : null);
  const committedMessages = steps.find(step => step.action === 'association-returned')?.facts.rpcMessages ?? messages;
  const operationIds = [...new Set(committedMessages.flatMap(message => message.events).filter(event => event.kind === 'association_stage' && typeof event.operation_id === 'string' && event.operation_id && typeof event.stage === 'string' && event.stage).map(event => event.operation_id))];
  // Correlation request data is retained in each step's runtime invocation trace;
  // choose the actual publication after the explicitly captured replacement step.
  const replacedStep = steps.find(step => step.action === 'session-replaced');
  const earlierCount = replacedStep ? (steps[steps.indexOf(replacedStep) - 1]?.facts.rpcMessages.length ?? 0) : messages.length;
  const replacementMessages = replacedStep ? replacedStep.facts.rpcMessages.slice(earlierCount) : [];
  const replacementSessionId = replacementMessages.flatMap(message => message.publications).at(-1)?.snapshot?.session?.sessionId ?? null;
  const currentStep = steps.find(step => step.action === 'current-operation');
  const currentStart = currentStep ? (steps[steps.indexOf(currentStep) - 1]?.facts.rpcMessages.length ?? 0) : messages.length;
  const currentMessages = currentStep ? currentStep.facts.rpcMessages.slice(currentStart) : [];
  const currentOperationId = currentMessages.flatMap(message => message.events).filter(event => event.kind === 'association_stage' && typeof event.operation_id === 'string' && event.operation_id && typeof event.stage === 'string' && event.stage).at(-1)?.operation_id ?? null;
  const currentSessionId = currentMessages.flatMap(message => message.publications).at(-1)?.snapshot?.session?.sessionId ?? null;
  return { protected_session_id: protectedSessionId, committed_operation_ids: operationIds, replacement_session_id: replacementSessionId, current_operation_id: currentOperationId, current_session_id: currentSessionId };
}
export function sceneChecks(scene, steps) {
  const checks = [], add = (id, expected, actual) => checks.push(assertion(id, expected, actual));
  const first = steps[0], last = steps.at(-1), before = steps.find(step => step.action === 'before-intent') ?? steps.find(step => step.action === 'target-reviewed') ?? first;
  const text = last.dom.text, button = expression => last.dom.buttons.filter(row => expression.test(row.label));
  const activeSources = sourceChoices(last), remembered = last.dom.checkboxes.filter(row => row.label.includes('记住'));
  const diff = (read, a = before, b = last) => read(b).length - read(a).length;
  const name = scene.name, production = first.facts.transportMode === 'production_rpc', inputs = actualSceneInputs(steps);
  const clicked = expression => (last.ui_actions ?? []).some(row => row.clicked && expression.test(row.label ?? ''));
  if (['library-global-owner','scoped-reconnect-owner'].includes(name)) {
    for(const [id,expected,actual] of libraryOwnerChecks(name,steps))add(id,expected,actual);
  } else if (name === 'library-window' || name === 'library-process') {
    add('library-is-entry', true, first.dom.headings.includes('游戏库'));
    add('candidate-selection-does-not-inject', 0, diff(step => callsOf(step, ['associate','beginReading'])));
    add('rust-review-is-read', true, callsOf(last, ['reviewAssociation']).length > 0);
    add('complete-identity-labels', true, ['目标完整路径','程序架构','Windows 游戏进程'].every(value => text.includes(value)));
    add('preview-failure-does-not-disable-hook', true, button(/^(关联并接入|切换并接入|复用连接)/).some(row => !row.disabled));
    if (name === 'library-process') add('process-list-selected', true, button(/^进程列表$/).some(row => row.pressed === 'true'));
  } else if (name === 'first-confirmation') {
    add('first-confirmation-label', true, button(/^选择对应的游戏窗口…$/).some(row => !row.disabled));
    add('first-confirmation-reason', true, last.dom.previews.some(row => /需要.*游戏窗口/.test(row.text)));
    add('no-invented-first-frame', true, last.dom.previews.every(row => !row.images.length && row.dateTime === null && row.time?.includes('尚无成功画面')));
  } else if (name === 'tcc') {
    add('tcc-explains-recording-permission', true, last.dom.previews.some(row => /屏幕录制权限|录屏权限/.test(row.text)));
    add('tcc-is-not-window-picker', true, !button(/^选择对应的游戏窗口…$/).length);
    add('hook-remains-independent', true, button(/^(关联并接入|切换并接入)/).some(row => !row.disabled));
  } else if (name === 'pairing-transfer') {
    const shared = steps.find(step => step.action === 'candidate-shared');
    add('candidate-had-decoded-frame', true, !!shared?.dom.previews.some(row => row.images.some(image => image.complete && image.naturalWidth > 0)));
    add('transfer-does-not-open-second-picker', 0, diff(step => callsOf(step, ['selectWindowPreview']), shared ?? before));
    add('session-has-decoded-frame', true, last.dom.previews.some(row => row.images.some(image => image.complete && image.naturalWidth > 0)));
    add('late-candidate-frame-is-not-session', true, steps.filter(step => step.action === 'session-frame' || step.action === 'late-candidate-frame').length === 2 && steps.find(step => step.action === 'session-frame').dom.previews[0]?.dateTime === last.dom.previews[0]?.dateTime);
  } else if (name === 'idle-refresh-details') {
    add('no-picker-from-refresh-or-details', 0, diff(step => callsOf(step, ['selectWindowPreview'])));
    add('no-association-from-refresh-or-details', 0, diff(step => callsOf(step, ['associate'])));
    add('closing-details-keeps-target-panel', true, last.dom.dialogs.length > 0);
  } else if (name === 'picker-cancel' || name === 'picker-timeout') {
    add('one-explicit-picker', 1, diff(step => callsOf(step, ['selectWindowPreview'])));
    add('picker-has-terminal-recovery', true, !last.dom.previews.some(row => ['starting','requesting_permission'].includes(row.state)) && /取消|超时|重新选择|选择对应/.test(text));
    add('picker-does-not-loop', true, diff(step => callsOf(step, ['selectWindowPreview'])) === 1);
  } else if (name === 'reuse') {
    add('same-target-is-not-switch', false, button(/^切换并接入$/).length > 0);
    add('reuse-explained', true, text.includes('复用'));
    add('relation-came-from-rust', true, callsOf(last, ['reviewAssociation']).length > 0);
    if (inputs.protected_session_id) add('reuse-preserves-actual-session', inputs.protected_session_id, last.facts.snapshot.session?.sessionId ?? null);
  } else if (name === 'switch-cancel' || name === 'cancel-candidate') {
    add('candidate-cancel-closes-observer', 0, last.dom.dialogs.length);
    add('candidate-cancel-action-executed', true, clicked(/^取消$/));
    if (inputs.protected_session_id) { add('reset-session-was-rendered', inputs.protected_session_id, first.facts.snapshot.session?.sessionId ?? null); add('candidate-cancel-keeps-session', inputs.protected_session_id, last.facts.snapshot.session?.sessionId ?? null); }
    add('candidate-cancel-does-not-inject', 0, diff(step => callsOf(step, ['associate','beginReading'])));
    if (name === 'switch-cancel') add('switch-impact-was-explicit', true, before.dom.text.includes('切换') && before.dom.text.includes('当前'));
  } else if (name === 'stale-review') {
    const replaced = steps.find(step => step.action === 'session-replaced');
    add('stale-review-action-executed', true, clicked(/^(关联并接入|切换并接入|复用连接)/));
    if (inputs.replacement_session_id) add('stale-review-keeps-new-session', inputs.replacement_session_id, last.facts.snapshot.session?.sessionId ?? null);
    add('stale-review-demands-refresh', true, /重新核对|已变化|已失效|过期/.test(text));
    add('fresh-review-was-requested', true, callsOf(last, ['reviewAssociation']).length > callsOf(before, ['reviewAssociation']).length);
  } else if (name === 'real-stage-order') {
    const pending = steps.find(step => step.action === 'association-returned');
    add('operation-stage-view-is-present', true, !!pending && pending.dom.progressStages.length >= 4);
    add('promise-is-not-handshake', true, !!pending && !pending.dom.progressStages.some(row => (row.current || row.complete) && /已连接|候选/.test(row.text)));
    add('stage-facts-have-origin', true, savedEffects(last).some(row => row.kind === 'association_stage' && typeof row.operation_id === 'string' && row.operation_id && typeof row.stage === 'string' && row.stage));
    add('core-stages-projected', true, ['窗口已关联','自动接入','选择正文'].every(label => steps.some(step => step.dom.text.includes(label))));
    add('handshake-fact-drives-connected-stage', true, steps.find(step => step.action === 'stage-handshake')?.dom.progressStages.some(row => row.current && /已连接/.test(row.text)) ?? false);
    add('candidate-fact-drives-choice-stage', true, candidateReadyEvidence(steps.find(step => step.action === 'stage-candidates_ready')));
  } else if (name === 'stale-events') {
    const current = steps.find(step => step.action === 'current-operation');
    add('old-operation-cannot-change-visible-stage', true, !!current && typeof current.dom.dialogs[0]?.text === 'string' && current.dom.dialogs[0].text.length > 0 && current.dom.dialogs[0].text === last.dom.dialogs[0]?.text);
    add('old-event-input-is-observed', true, savedEffects(last).some(row => row.kind === 'association_event_rejected' && typeof row.operation_id === 'string' && row.operation_id));
    if (inputs.current_session_id) add('old-operation-cannot-replace-session', inputs.current_session_id, last.facts.snapshot.session?.sessionId ?? null);
  } else if (name === 'close-operation') {
    add('closing-observer-does-not-cancel-operation', 0, last.facts.methodCalls.filter(row => /cancel.*association|disconnect/i.test(row.method) || row.method === 'dispatch' && /disconnect|stop_capture/.test(row.args[0]?.command?.kind ?? '')).length);
    add('close-operation-was-actually-associated', 1, callsOf(last, ['associate']).length);
    if (inputs.committed_operation_ids.length) {
      add('observer-can-close-while-operation-owned', 0, last.dom.dialogs.length);
      const beforeEvents = before.facts.rpcMessages.flatMap(message => message.events).filter(event => event.kind === 'association_stage');
      const continuedEvents = last.facts.rpcMessages.slice(before.facts.rpcMessages.length).flatMap(message => message.events);
      add('real-committed-operation-advances-after-observer-closes', true, continuedEvents.some(row => row.kind === 'association_stage' && inputs.committed_operation_ids.includes(row.operation_id) && typeof row.stage === 'string' && row.stage && row.stage !== beforeEvents.filter(event => event.operation_id === row.operation_id).at(-1)?.stage));
    }
  } else if (name === 'empty') {
    add('source-controls-are-multi-select', true, sourceChoices(last).length >= 2);
    add('no-default-selected-source', 0, activeSources.filter(row => row.checked).length);
    add('empty-begin-reading-disabled', true, button(/^开始阅读$/).some(row => row.disabled));
    add('empty-selection-has-no-commit', 0, commitsOf(last).length);
  } else if (name === 'draft-later') {
    add('source-controls-are-multi-select', true, sourceChoices(before).length >= 2);
    add('draft-and-later-do-not-commit', 0, diff(commitsOf));
    add('draft-and-later-have-no-window-effects', 0, diff(step => callsOf(step, ['openWindow','closeWindow'])));
    add('later-keeps-connected-session', before.facts.snapshot.session?.sessionId ?? null, last.facts.snapshot.session?.sessionId ?? null);
    add('later-action-executed', true, clicked(/^稍后选择$/));
    add('later-closes-only-panel', 0, last.dom.dialogs.length);
  } else if (['commit-set','commit-failure','owned-remember','unowned-remember','temporary-saved','requires-review','effects-order','effects-retry','stale-effects','submit-retry','source-expired'].includes(name)) {
    if (name === 'unowned-remember') {
      add('unowned-remember-explicitly-disabled', true, remembered.length === 1 && remembered[0].disabled && /游戏|归属/.test(text));
      add('unowned-does-not-silently-save', 0, commitsOf(last).length);
    } else if (name === 'requires-review') {
      add('unstable-saved-choice-demands-review', true, /重新核对|需要核对/.test(text));
      add('requires-review-never-auto-selects', 0, activeSources.filter(row => row.checked).length);
      add('requires-review-never-auto-commits', 0, commitsOf(last).length);
    } else {
      const selected = steps.find(step => step.action === 'draft-selected');
      add('two-source-draft-is-visible', true, !!selected && sourceChoices(selected).filter(row => row.checked).length === 2);
      add('no-submit-before-explicit-intent', 0, selected ? diff(commitsOf, before, selected) : 0);
      const selectedIds = selected ? sourceChoices(selected).filter(row => row.checked).map(row => row.sourceId).sort() : [];
      add('explicit-begin-action-executed', true, clicked(/^开始阅读$/));
      if (production) add('selected-source-identities-are-observable', true, selectedIds.length === 2 && selectedIds.every(value => typeof value === 'string' && value) && new Set(selectedIds).size === 2);
      if (['commit-failure','submit-retry','source-expired'].includes(name)) {
        const failed = steps.find(step => step.action === 'submit-failed') ?? last;
        add('failed-submit-retains-draft', true, sourceChoices(failed).filter(row => row.checked).length === 2);
        if (production) add('failed-submit-keeps-same-source-identities', selectedIds, sourceChoices(failed).filter(row => row.checked).map(row => row.sourceId).sort());
        add('failed-submit-has-error', true, /失败|未完成|失效|重试/.test(failed.dom.text));
        add('failed-submit-has-no-window-effects', 0, diff(step => callsOf(step, ['openWindow','closeWindow']), before, failed));
      }
      if (production && !['commit-failure','source-expired'].includes(name)) {
        add('one-effective-selection-commit', 1, savedEffects(last).filter(row => row.kind === 'selection_commit').length);
        const commit = savedEffects(last).find(row => row.kind === 'selection_commit');
        add('selection-commit-matches-checked-members', selectedIds, Array.isArray(commit?.source_ids) ? [...commit.source_ids].sort() : []);
      }
      if (name === 'owned-remember') { add('owned-remember-has-explicit-choice', true, !!selected?.dom.checkboxes.some(row => row.label.includes('记住') && row.checked && !row.disabled)); if (production) add('remember-intent-was-committed', true, savedEffects(last).some(row => row.kind === 'selection_commit' && row.remember === true)); }
      if (name === 'temporary-saved') { add('temporary-intent-was-committed', true, savedEffects(last).some(row => row.kind === 'selection_commit' && row.remember === false)); add('temporary-preserves-saved-descriptor', true, savedEffects(last).some(row => row.kind === 'saved_selection_comparison' && row.before_hash === row.after_hash && typeof row.before_hash === 'string')); }
      if (name === 'effects-order') {
        const kinds = savedEffects(last).map(row => row.kind);
        add('persist-before-show-before-hide', true, kinds.indexOf('selection_commit') >= 0 && kinds.indexOf('selection_commit') < kinds.indexOf('hud_shown') && kinds.indexOf('hud_shown') < kinds.indexOf('main_hidden'));
      }
      if (name === 'effects-retry') {
        const failed = steps.find(step => step.action === 'effect-failed');
        add('effect-failure-is-distinct', true, !!failed && /选择已保存|已保存选择/.test(failed.dom.text) && /窗口|浮窗/.test(failed.dom.text));
        add('effect-retry-does-not-resubmit', 0, failed ? diff(commitsOf, failed) : commitsOf(last).length);
        add('effect-retry-is-observed', true, savedEffects(last).some(row => row.kind === 'hud_shown') && savedEffects(last).some(row => row.kind === 'main_hidden'));
      }
      if (name === 'stale-effects') {
        const replaced = steps.find(step => step.action === 'session-replaced');
        if (inputs.replacement_session_id) add('old-effects-cannot-change-new-session', inputs.replacement_session_id, last.facts.snapshot.session?.sessionId ?? null);
        add('stale-effect-attempt-was-rejected', true, savedEffects(last).some(row => row.kind === 'stale_window_effect_rejected'));
      }
    }
  } else if (name === 'details') {
    add('full-sample-retained', true, last.facts.session?.sources?.every(source => !source.sample || text.includes(source.sample)) ?? false);
    const details = last.source_details;
    for (const row of details.rows) {
      add('source-row-' + row.source_id, 1, row.rowCount);
      for (const field of row.fields) if (field.inputAvailable) add('source-field-' + row.source_id + '-' + field.label, field.expected, field.matchCount === 1 ? field.actual : null);
    }
  } else if (name.startsWith('layout-') || name === 'other-bottle-warning') {
    add('root-does-not-scroll', true, steps.every(step => step.dom.geometry.bodyScrollTop === 0 && step.dom.geometry.rootScrollTop === 0));
    add('no-root-horizontal-overflow', true, steps.every(step => step.dom.geometry.rootScrollWidth <= step.dom.geometry.rootClientWidth + 1));
    add('identity-preview-confirmation-reachable', true, ['identity-reachable','preview-reachable','confirmation-reachable'].every(action => steps.some(step => step.action === action)));
    add('confirmation-is-in-viewport', true, last.dom.buttons.some(row => /^(关联并接入|切换并接入|复用连接)/.test(row.label) && row.rect.x >= 0 && row.rect.y >= 0 && row.rect.right <= last.dom.geometry.width + 1 && row.rect.bottom <= last.dom.geometry.height + 1));
    if (name === 'other-bottle-warning') add('other-bottle-warning-is-secondary-disclosure', true, last.dom.details.some(row => /容器|Bottle|其他/.test(row.text) && !row.open));
  } else if (name === 'frame-state' || name === 'revoke' || name === 'late-metadata') {
    const live = steps.find(step => step.action === 'valid-frame'), preview = step => step?.dom.previews[0];
    add('real-frame-decodes', true, !!preview(live)?.images.some(image => image.complete && image.naturalWidth > 0));
    if (name === 'revoke') {
      add('revocation-clears-image', 0, preview(last)?.images.length ?? 0);
      add('revocation-explained', true, /撤销|录屏|屏幕录制/.test(preview(last)?.text ?? ''));
      if (inputs.protected_session_id) add('revocation-does-not-disconnect-hook', inputs.protected_session_id, last.facts.snapshot.session?.sessionId ?? null);
      add('revocation-sends-no-disconnect', 0, last.facts.methodCalls.filter(row => /disconnect|stopCapture/.test(row.method) || row.method === 'dispatch' && /disconnect|stop_capture/.test(row.args[0]?.command?.kind ?? '')).length);
    } else if (name === 'late-metadata') {
      add('metadata-or-old-frame-cannot-advance-time', preview(live)?.dateTime ?? null, preview(last)?.dateTime ?? null);
      add('metadata-does-not-claim-recovery', true, !/实时预览/.test(preview(last)?.text ?? ''));
    } else {
      const paused = steps.find(step => step.action === 'paused'), resuming = steps.find(step => step.action === 'resuming'), newer = steps.find(step => step.action === 'new-frame');
      add('pause-control-was-invoked', 1, last.facts.methodCalls.filter(row => row.method === 'controlWindowPreview' && row.args[0]?.action === 'pause').length);
      add('resume-control-was-invoked', 1, last.facts.methodCalls.filter(row => row.method === 'controlWindowPreview' && row.args[0]?.action === 'resume').length);
      add('pause-state-was-visible', 'paused', preview(paused)?.state ?? null);
      add('valid-frame-had-a-timestamp', true, typeof preview(live)?.dateTime === 'string' && !!preview(live)?.dateTime);
      add('pause-keeps-frame-time', preview(live)?.dateTime ?? null, preview(paused)?.dateTime ?? null);
      add('resume-without-frame-keeps-time', preview(live)?.dateTime ?? null, preview(resuming)?.dateTime ?? null);
      add('new-frame-advances-time', true, !!preview(newer)?.dateTime && preview(newer)?.dateTime !== preview(live)?.dateTime);
    }
  } else if (name === 'association-retry' || name === 'source-query-retry') {
    const failed = steps.find(step => step.action === 'operation-failed');
    add('failure-is-visible', true, !!failed && /失败|未完成|重试/.test(failed.dom.text));
    add('retry-remains-explicit', true, !!failed && failed.dom.buttons.some(row => /重试/.test(row.label) && !row.disabled));
    add('target-retained-on-failure', true, !!failed && /目标完整路径|窗口已关联|选择正文/.test(failed.dom.text));
    if (name === 'association-retry') add('retry-reviews-current-target', true, callsOf(last, ['reviewAssociation']).length >= 2);
    else add('source-query-retry-does-not-reinject', 0, failed ? diff(step => callsOf(step, ['associate']), failed) : 1);
  }
  return checks;
}

async function main() {
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const desktop = path.join(root, 'apps/desktop');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const now = () => new Date().toISOString();
const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const name = process.argv[index], value = process.argv[index + 1];
  assert.ok(['--output', '--nonce', '--projection-fixtures'].includes(name) && value && !options.has(name), 'exact unique runner arguments required');
  options.set(name, value);
}
assert.equal(options.size, 3, '--output, --nonce and --projection-fixtures are required');
const nonce = options.get('--nonce');
assert.match(nonce, /^[A-Za-z0-9_.-]{1,128}$/);
const output = path.resolve(options.get('--output'));
assert.ok(output.startsWith(path.join(root, '.build') + path.sep), 'output must be fresh ignored evidence');
async function noSymlinks(file) {
  for (let current = file; current !== root && current !== path.dirname(current); current = path.dirname(current)) {
    try { assert.equal((await fs.lstat(current)).isSymbolicLink(), false, 'symlink evidence paths forbidden'); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
}
await noSymlinks(output);
await fs.mkdir(output, { recursive: false });
const reference = async file => ({ path: path.relative(root, file).split(path.sep).join('/'), sha256: hash(await fs.readFile(file)) });
const json = async (name, value) => { const file = path.join(output, name); await fs.writeFile(file, JSON.stringify(value, null, 2) + '\n'); return reference(file); };
function productionIdentity() {
  const result = spawnSync('python3', ['-c', 'import sys,json;from pathlib import Path;sys.path.insert(0,str(Path(sys.argv[1])/"scripts/quality"));from implementation_status import product_source_identity;print(json.dumps(product_source_identity(Path(sys.argv[1])),sort_keys=True))', root], { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}
const started = now(), sourceBefore = productionIdentity(), cases = [], infrastructureErrors = [], screenshots = [], browserErrors = [], browserConsole = [], pageErrors = [], commandExecutions = [];
const securityFile = 'apps/desktop/src-tauri/tauri.conf.json';
const securityBytes = await fs.readFile(path.join(root, securityFile));
const productionCsp = JSON.parse(securityBytes).app.security.csp;
assert.equal(typeof productionCsp, 'string'); assert.ok(productionCsp.length > 0);
const securityPolicy = { source_file: securityFile, source_sha256: hash(securityBytes), csp: productionCsp, delivery: 'document_response_header', violations: [] };
const manifestPath = path.resolve(options.get('--projection-fixtures'));
await noSymlinks(manifestPath);
assert.ok(manifestPath.startsWith(path.join(root, '.build') + path.sep));
const manifestBytes = await fs.readFile(manifestPath), manifest = JSON.parse(manifestBytes);
assert.equal(manifest.kind, 'binding-journey-production-projections/v1');
assert.equal(manifest.schema_version, 1);
assert.equal(manifest.nonce, nonce);
assert.equal(manifest.status, 'passed');
assert.deepEqual(manifest.infrastructure_errors, []);
assert.deepEqual(manifest.source_before, sourceBefore);
assert.deepEqual(manifest.source_after, sourceBefore);
const projectionPath = path.resolve(root, manifest.projection_file.path);
await noSymlinks(projectionPath);
assert.ok(projectionPath.startsWith(path.dirname(manifestPath) + path.sep));
assert.equal(hash(await fs.readFile(projectionPath)), manifest.projection_file.sha256);
const productionValues = JSON.parse(await fs.readFile(projectionPath, 'utf8'));
assert.equal(productionValues.nonce, nonce);
assert.equal(productionValues.selection_required.errorCode, 'preview_window_selection_required');
for (const [file, expected] of Object.entries(manifest.sources)) assert.equal(hash(await fs.readFile(path.join(root, file))), expected, file);
const plan = JSON.parse(await fs.readFile(path.join(root, 'docs/development/binding-journey-plan.json'), 'utf8'));
assert.deepEqual(plan.cases.map(row => row.id), Array.from({ length: 18 }, (_, i) => 'BJ-' + String(i + 1).padStart(2, '0')));
const producerManifest = await reference(manifestPath);

// Optional future transport exchanges must be emitted by a production fixture
// producer and inspected separately. No business DTO shape is invented here.
const transport = manifest.runtime_transport ?? null;
let transportExchanges = null;
if (transport) {
  assert.equal(transport.kind, 'binding-journey-runtime-transport/v1');
  assert.equal(transport.nonce, nonce);
  assert.ok(transport.producer && transport.compilation && transport.inventory && transport.binary, 'runtime producer requires registered compilation, inventory and binary evidence');
  if (transport.mode === 'ndjson') {
    assert.equal(transport.protocol, 'LUNA_BINDING_RPC_V1');
    const binary = path.resolve(root, transport.binary.path); await noSymlinks(binary);
    assert.equal(hash(await fs.readFile(binary)), transport.binary.sha256);
  } else {
    const file = path.resolve(root, transport.file.path); await noSymlinks(file);
    assert.equal(hash(await fs.readFile(file)), transport.file.sha256);
    transportExchanges = JSON.parse(await fs.readFile(file, 'utf8'));
    assert.equal(transportExchanges.nonce, nonce);
    assert.ok(transport.execution, 'static responses require actual producer execution evidence');
  }
}

function pointerValue(value, pointer) {
  assert.equal(typeof pointer, 'string');
  if (pointer === '') return value;
  assert.ok(pointer.startsWith('/'), 'serialized-response JSON pointer must be absolute');
  const parts = pointer.slice(1).split('/'); assert.ok(parts.every(value => !/~(?![01])/.test(value)), 'invalid JSON pointer escape');
  for (const part of parts.map(value => value.replace(/~1/g, '/').replace(/~0/g, '~'))) {
    assert.ok(value !== null && typeof value === 'object' && Object.hasOwn(value, part), 'source-detail JSON pointer is absent');
    value = value[part];
  }
  return value;
}
const sourceDetailObligations = transportExchanges?.source_detail_obligations ?? [];
assert.ok(Array.isArray(sourceDetailObligations));
const sourceIds = new Set();
for (const row of sourceDetailObligations) {
  assert.ok(typeof row.source_id === 'string' && row.source_id && !sourceIds.has(row.source_id)); sourceIds.add(row.source_id);
  assert.ok(Array.isArray(row.fields) && row.fields.length > 0);
  const labels = new Set();
  for (const field of row.fields) {
    assert.ok(['Hook', 'Address', 'Context', 'Context 2'].includes(field.label) && !labels.has(field.label)); labels.add(field.label);
    assert.equal(typeof field.value, 'string', 'u64-capable source detail must retain its serialized string');
    assert.ok(Number.isSafeInteger(field.exchange_index) && field.exchange_index >= 0);
    const response = transportExchanges.methods?.[field.method]?.[field.exchange_index]?.response;
    assert.equal(pointerValue(response, field.json_pointer), field.value, 'source detail must come from its actual serialized response');
  }
}

let browser, browserIdentity, server, serverLog = '', origin, responseHeadersRef = null, rpc = null, rpcReady = null;
const sceneResults = [], decodedImageEvidence = new Map();
let mountedPageCount = 0;
let activeDiagnosticScene = null;
const diagnosticScenes = {
  'BJC-01-system-confirmation-state':'BJS-02-first-confirmation',
  'BJC-02-same-target-reuse':'BJS-05-reuse',
  'BJC-03-association-progress':'BJS-06-real-stage-order',
  'BJC-04-selection-draft-confirmation':'BJS-08-draft-later',
  'BJC-05-source-details':'BJS-10-details',
  'BJC-06-remember-selection':'BJS-09-owned-remember',
  'BJC-07-no-unrequested-picker':'BJS-04-idle-refresh-details',
};
const diagnosticMethods = {
  'BJC-01-system-confirmation-state':['snapshot','targets','reviewAssociation','previewCapability','startWindowPreview'],
  'BJC-02-same-target-reuse':['snapshot','targets','reviewAssociation','previewCapability','startWindowPreview'],
  'BJC-03-association-progress':['snapshot','targets','reviewAssociation','associate','associationProgress','previewCapability','startWindowPreview'],
  'BJC-04-selection-draft-confirmation':['snapshot','targets','reviewAssociation','associate','associationProgress','sourceSelection','beginReading','previewCapability','startWindowPreview'],
  'BJC-05-source-details':['snapshot','targets','reviewAssociation','associate','associationProgress','sourceSelection','previewCapability','startWindowPreview'],
  'BJC-06-remember-selection':['snapshot','targets','reviewAssociation','associate','associationProgress','sourceSelection','beginReading','previewCapability','startWindowPreview'],
  'BJC-07-no-unrequested-picker':['snapshot','targets','reviewAssociation','previewCapability','startWindowPreview','selectWindowPreview','controlWindowPreview'],
};
async function execute(args, name, cwd = desktop) {
  const begin = now();
  const result = await new Promise(resolve => {
    const child = spawn(args[0], args.slice(1), { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '', timer;
    child.stdout.on('data', data => stdout += data);
    child.stderr.on('data', data => stderr += data);
    child.on('error', error => { clearTimeout(timer); resolve({ exit_code: -1, stdout, stderr: stderr + String(error) }); });
    child.on('close', code => { clearTimeout(timer); resolve({ exit_code: code, stdout, stderr }); });
    timer = setTimeout(() => { stderr += '\nRunner command timed out'; child.kill('SIGTERM'); }, 180000);
  });
  const stdout = path.join(output, name + '.stdout'), stderr = path.join(output, name + '.stderr');
  await fs.writeFile(stdout, result.stdout); await fs.writeFile(stderr, result.stderr);
  const artifact = await json(name + '.execution.json', { command: args, cwd: path.relative(root, cwd), started_at: begin, finished_at: now(), exit_code: result.exit_code, stdout: await reference(stdout), stderr: await reference(stderr) });
  commandExecutions.push(artifact);
  assert.equal(result.exit_code, 0, name + ' infrastructure failed: ' + result.stderr);
}
async function serializeLiveDOM(page, captured = null) {
  captured ??= await page.evaluate(captureLiveDocument);
  for (const [index, image] of captured.images.entries()) {
    assert.ok(image.dataUrl.startsWith('data:image/png;base64,'));
    const bytes = Buffer.from(image.dataUrl.slice('data:image/png;base64,'.length), 'base64'), digest = hash(bytes);
    const file = path.join(output, 'decoded-image-' + digest + '.png');
    try { await fs.writeFile(file, bytes, { flag: 'wx' }); } catch (error) { if (error.code !== 'EEXIST') throw error; assert.equal(hash(await fs.readFile(file)), digest); }
    const evidence = { ...await reference(file), original_source: image.originalSource, natural_width: image.width, natural_height: image.height, encoding: 'png', conversion: 'actual-decoded-pixels-1:1-no-resize' };
    decodedImageEvidence.set(image.originalSource + ':' + digest, evidence);
    captured.html = captured.html.replace('data-binding-decoded-image-index="' + index + '"', 'data-binding-decoded-image-path="' + evidence.path + '" data-binding-decoded-image-sha256="' + digest + '"');
  }
  return captured.html;
}

function capturePendingAssociationState() {
  const visible=node=>{const r=node.getBoundingClientRect(),s=getComputedStyle(node);return r.width>0&&r.height>0&&s.display!=='none'&&s.visibility!=='hidden';};
  const dialogs=[...document.querySelectorAll('[role="dialog"]')].filter(visible);
  if(dialogs.length!==1)throw Error('one actual visible association dialog required');
  const dialog=dialogs[0],facts=window.bindingHarness.facts();
  const stageLabels=[...dialog.querySelectorAll('ol[aria-label="接入流程"] li')].filter(visible).map(node=>{const copy=node.cloneNode(true);for(const index of copy.querySelectorAll('b,[aria-hidden="true"]'))index.remove();return copy.textContent.trim();});
  const duplicateSubmitAvailable=[...dialog.querySelectorAll('button')].some(button=>visible(button)&&!button.disabled&&button.getAttribute('aria-disabled')!=='true'&&/^(关联并接入|切换并接入|复用连接)$/.test(button.textContent.trim()));
  return {at_ms:Date.now(),ui:{visibleText:dialog.textContent,stageLabels,dialogVisible:visible(dialog),duplicateSubmitAvailable},facts,serialized:window.bindingHarness.captureDocument()};
}

async function saveCase(page, id, observations, assertions, events, captured = null) {
  const basename = id.toLowerCase(), png = path.join(output, basename + '.png'), dom = path.join(output, basename + '.html');
  await page.evaluate(() => document.fonts.ready);
  if(captured?.screenshot)await fs.writeFile(png,await fs.readFile(captured.screenshot),{flag:'wx'});
  else await page.screenshot({ path: png, animations: 'disabled' });
  await fs.writeFile(dom, await serializeLiveDOM(page,captured?.serialized??null));
  const trace = await json(basename + '.trace.json', { schema_version: 1, kind: 'binding-journey-case-observations/v1', nonce, case_id: id, observations, events });
  const screenshot = await reference(png), domRef = await reference(dom);
  securityPolicy.violations.push(...await page.evaluate(() => window.bindingCspViolations ?? []));
  screenshots.push(screenshot);
  const value = { id, status: !assertions.every(row => row.passed) ? 'failed' : observations.blocked_obligations?.length ? 'blocked' : 'passed', assertions, observations, trace, screenshot, dom: domRef };
  cases.push(value);
  console.log(id + ': ' + value.status + ' (' + assertions.filter(row => !row.passed).map(row => row.id).join(', ') + ')');
}
async function rawFacts(page) { return page.evaluate(() => window.bindingHarness.facts()); }
async function openPage(scenario = 'empty', viewport = { width: 1040, height: 720 }, zoom = 1, rpcScene = null) {
  rpcScene ??= activeDiagnosticScene;
  const page = await browser.newPage({ viewport, deviceScaleFactor: 1 });
  page.setDefaultTimeout(5000);
  page.on('pageerror', error => { const value = { at: now(), url: page.url(), name: error.name, message: error.message, stack: error.stack }; pageErrors.push(value); browserErrors.push(value); });
  page.on('console', message => { const value = { at: now(), url: page.url(), type: message.type(), text: message.text(), location: message.location() }; browserConsole.push(value); if (message.type() === 'error') browserErrors.push({ name: 'console.error', ...value, message: message.text() }); });
  await page.addInitScript(() => {
    window.bindingCspViolations = [];
    document.addEventListener('securitypolicyviolation', event => window.bindingCspViolations.push({ blockedURI: event.blockedURI, violatedDirective: event.violatedDirective, effectiveDirective: event.effectiveDirective, disposition: event.disposition }));
  });
  await page.route('**/*', route => { const url = new URL(route.request().url()); return ['data:', 'blob:', 'about:'].includes(url.protocol) || url.origin === origin ? route.continue() : route.abort(); });
  if (rpcScene) await page.exposeBinding('__lunaBindingRpc', async (_source, request) => { assert.equal(request.scenario, rpcScene); return rpc.request(rpcScene, request.action, request.method, request.args); });
  await page.goto(origin + '/?scenario=' + scenario + (rpcScene ? '&rpcScene=' + encodeURIComponent(rpcScene) : ''));
  await page.waitForFunction(() => !!window.bindingHarness && document.querySelector('nav[aria-label="主导航"]'));
  await page.getByRole('heading', { name: '游戏库', exact: true }).waitFor(); mountedPageCount++;
  if (zoom !== 1) await page.evaluate(value => document.documentElement.style.zoom = String(value), zoom);
  return page;
}
async function openTarget(page) {
  await page.getByRole('button', { name: '连接运行中的游戏…', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('radio').first().check();
  await dialog.getByRole('region', { name: '运行窗口预览', exact: true }).locator('[data-preview-state]').count();
  await page.waitForFunction(() => window.bindingHarness.facts().methodCalls.some(row => ['startWindowPreview','reviewAssociation'].includes(row.method)));
  await dialog.getByRole('region', { name: '运行窗口预览', exact: true }).scrollIntoViewIfNeeded();
  return dialog;
}
async function openSources(page) {
  const dialog = await openTarget(page);
  await dialog.getByRole('button', { name: /^(关联并接入|切换并接入)$/ }).click();
  await page.waitForFunction(() => (window.bindingHarness.facts().session?.sources?.length ?? 0) > 0);
  const sourceId = await page.evaluate(() => window.bindingHarness.facts().session.sources[0].sourceId);
  await page.waitForFunction(id => [...document.querySelectorAll('[role="dialog"] [data-source-id]')].filter(node => node.dataset.sourceId === id).length === 1, sourceId);
  const escaped=await page.evaluate(id=>CSS.escape(id),sourceId);
  const matched=dialog.locator(`[data-source-id="${escaped}"]`);
  assert.equal(await matched.count(),1,'one exact typed sourceId row required');
  const checkbox = matched.getByRole('checkbox');
  assert.equal(await checkbox.count(),1); await checkbox.waitFor({state:'visible'});
  return dialog;
}
async function check(id, fn) {
  if (diagnosticScenes[id] && transport?.mode === 'ndjson') {
    const missing = !rpcReady ? ['producer-ready'] : [
      ...rpcReady.capabilities.scene_ids.includes(diagnosticScenes[id]) ? [] : ['scene:' + diagnosticScenes[id]],
      ...[...new Set([...diagnosticMethods[id], 'controlWindowPreview'])].filter(method => !rpcReady.capabilities.methods.includes(method)).map(method => 'method:' + method),
      ...rpcReady.capabilities.controls.includes('reset') ? [] : ['control:reset'],
    ];
    if (missing.length) {
      const observations = { transport_mode: 'unavailable', blocked_obligations: missing };
      const trace = await json(id.toLowerCase() + '.trace.json', { schema_version: 1, kind: 'binding-journey-case-observations/v1', nonce, case_id: id, observations, events: [] });
      cases.push({ id, status: 'blocked', assertions: [], observations, trace, screenshot: null, dom: null }); return;
    }
    activeDiagnosticScene = diagnosticScenes[id];
  }
  try { await fn(); }
  catch (error) { infrastructureErrors.push({ case_id: id, message: error.message, stack: error.stack }); console.error(id + ': infrastructure error: ' + error.message); }
  finally { activeDiagnosticScene = null; }
}

async function sourceDetailObservation(page, facts) {
  const reset = facts.rpcMessages?.find(row => row.action === 'reset' && row.outcome === 'ok');
  const metadata = reset?.response?.source_detail_obligations ?? [];
  const rows = [], missing = [];
  assert.ok(Array.isArray(metadata), 'source-detail evidence metadata must be a list');
  for (const row of metadata) {
    const origin = row.response_id ? facts.rpcMessages.find(message => message.id === row.response_id) : reset;
    if (!origin) { missing.push('source-detail-response:' + row.source_id); continue; }
    const value = row.response_id ? origin.response : origin.response?.initial_snapshot;
    assert.ok(typeof row.source_pointer === 'string' && row.source_pointer.startsWith('/') && row.source_pointer.length > 1, 'source evidence must point to a concrete entity below the response root');
    assert.ok(typeof row.source_id_pointer === 'string' && /^\/(?:[^/~]|~[01])+$/.test(row.source_id_pointer), 'source ID pointer must be one direct scalar field');
    const source = pointerValue(value, row.source_pointer);
    assert.ok(source && typeof source === 'object' && !Array.isArray(source), 'source evidence entity must be a single object');
    assert.equal(typeof pointerValue(source, row.source_id_pointer), 'string', 'source ID must be a scalar string');
    assert.equal(pointerValue(source, row.source_id_pointer), row.source_id, 'source-detail metadata must identify its actual source object');
    assert.ok(Array.isArray(row.fields) && row.fields.length > 0);
    const fields = [];
    for (const field of row.fields) {
      assert.ok(['Hook','Address','Context','Context 2'].includes(field.label));
      assert.ok(field.json_pointer.startsWith(row.source_pointer === '' ? '/' : row.source_pointer + '/'), 'source field escaped its source subtree');
      const expected = pointerValue(value, field.json_pointer);
      assert.deepEqual(expected, field.value, 'source expected value differs from actual production response');
      fields.push({ ...field, expected, inputAvailable: typeof expected === 'string', responseId: origin.id });
    }
    rows.push({ source_id: row.source_id, source_pointer: row.source_pointer, source_id_pointer: row.source_id_pointer, response_id: origin.id, fields });
  }
  const observed = await page.evaluate(rows => {
    const visible = node => {
      if (!node || !node.getClientRects().length || node.getBoundingClientRect().width <= 0 || node.getBoundingClientRect().height <= 0) return false;
      for (let parent = node; parent; parent = parent.parentElement) { const style = getComputedStyle(parent); if (parent.hidden || style.display === 'none' || ['hidden','collapse'].includes(style.visibility) || Number(style.opacity) === 0) return false; }
      return true;
    };
    return rows.map(row => {
      const candidates = [...document.querySelectorAll('[role="dialog"] [data-source-id]')].filter(node => node.dataset.sourceId === row.source_id);
      return { ...row, rowCount: candidates.length, fields: row.fields.map(field => {
        const labels = candidates.length === 1 ? [...candidates[0].querySelectorAll('dt')].filter(node => visible(node) && node.textContent.trim() === field.label) : [];
        const definitions = labels.map(node => node.nextElementSibling).filter(node => node?.tagName === 'DD' && visible(node));
        return { ...field, matchCount: definitions.length, actual: definitions.length === 1 ? definitions[0].textContent.trim() : null };
      }) };
    });
  }, rows);
  const advancedAvailable = observed.length > 0 && observed.every(row => ['Hook','Address','Context','Context 2'].every(label => row.fields.some(field => field.label === label && field.inputAvailable)));
  const precisionAvailable = observed.some(row => row.fields.some(field => ['Address','Context','Context 2'].includes(field.label) && typeof field.expected === 'string' && /^(0|[1-9]\d*)$/.test(field.expected) && BigInt(field.expected) > 9007199254740991n && BigInt(field.expected) <= 18446744073709551615n));
  return { rows: observed, metadata, advancedAvailable, precisionAvailable, blocked_obligations: [...missing, ...advancedAvailable ? [] : ['advanced-source-typed-DTO-disclosure'], ...precisionAvailable ? [] : ['source-u64-typed-DTO-projection']] };
}
let sceneStepSequence = 1, activeSceneActions = [];
async function sceneSnapshot(page, action) {
  await page.evaluate(() => document.fonts.ready);
  const checkpoint = await page.evaluate(() => {
    const visible = node => !!node && !node.closest('[hidden]') && getComputedStyle(node).display !== 'none' && getComputedStyle(node).visibility !== 'hidden' && node.getBoundingClientRect().width > 0 && node.getBoundingClientRect().height > 0;
    const label = node => node.getAttribute('aria-label') || [...(node.labels ?? [])].map(item => item.innerText).join(' ') || node.innerText || node.getAttribute('title') || '';
    const rect = node => node.getBoundingClientRect().toJSON();
    const buttons = [...document.querySelectorAll('button,[role="button"]')].filter(visible).map(node => ({ label: label(node).trim(), disabled: node.matches(':disabled') || node.getAttribute('aria-disabled') === 'true', pressed: node.getAttribute('aria-pressed'), rect: rect(node) }));
    const checkboxes = [...document.querySelectorAll('input[type="checkbox"],[role="checkbox"]')].filter(visible).map(node => ({ label: label(node).trim(), checked: node.checked ?? node.getAttribute('aria-checked') === 'true', disabled: node.matches(':disabled') || node.getAttribute('aria-disabled') === 'true', sourceId: node.getAttribute('data-source-id') ?? node.closest('[data-source-id]')?.getAttribute('data-source-id') ?? (node.value !== 'on' ? node.value : null) }));
    const sourceRows=[...document.querySelectorAll('[role="dialog"]')].filter(visible).flatMap(dialog=>[...dialog.querySelectorAll('[data-source-id]')].map(node=>{
      const controls=[...node.querySelectorAll('input[type="checkbox"],[role="checkbox"]')];
      return {sourceId:node.dataset.sourceId,checkboxCount:controls.length,visibleCheckboxCount:controls.filter(visible).length,enabledCheckboxCount:controls.filter(control=>visible(control)&&!control.matches(':disabled')&&control.getAttribute('aria-disabled')!=='true').length};
    }));
    const previews = [...document.querySelectorAll('[aria-label="运行窗口预览"]')].filter(visible).map(node => ({ state: node.getAttribute('data-preview-state'), text: node.innerText, rect: rect(node), time: node.querySelector('time')?.textContent ?? null, dateTime: node.querySelector('time')?.getAttribute('datetime') ?? null, images: [...node.querySelectorAll('img')].map(image => ({ alt: image.alt, naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight, complete: image.complete })) }));
    const dialogs = [...document.querySelectorAll('[role="dialog"]')].filter(visible).map(node => ({ text: node.innerText, rect: rect(node), ariaLabel: node.getAttribute('aria-label') }));
    const scrollOwners = [...document.querySelectorAll('*')].filter(node => visible(node) && ['auto','scroll'].includes(getComputedStyle(node).overflowY) && node.scrollHeight > node.clientHeight + 1).map(node => ({ tag: node.tagName, role: node.getAttribute('role'), key: node.getAttribute('data-ui-scroll'), rect: rect(node), scrollTop: node.scrollTop, scrollHeight: node.scrollHeight, clientHeight: node.clientHeight }));
    const progressStages = [...document.querySelectorAll('[aria-label="自动接入阶段"] li')].filter(visible).map(node => ({ text: node.innerText.trim(), current: node.getAttribute('aria-current') === 'step' || /(?:^|[_ -])current(?:[_ -]|$)/.test(node.className), complete: /(?:^|[_ -])complete(?:[_ -]|$)/.test(node.className), className: node.className, ariaCurrent: node.getAttribute('aria-current') }));
    const fontSamples = [...document.querySelectorAll('h1,h2,button,input,[aria-label="运行窗口预览"]')].filter(visible).slice(0,80).map(node => { const style = getComputedStyle(node); return { tag: node.tagName, label: label(node).trim().slice(0,120), family: style.fontFamily, size: style.fontSize, weight: style.fontWeight, lineHeight: style.lineHeight, letterSpacing: style.letterSpacing }; });
    return { dom: { text: document.body.innerText, progressStages, sourceRows, fonts: { ready: document.fonts.status, samples: fontSamples }, buttons, checkboxes, previews, dialogs, scrollOwners, headings: [...document.querySelectorAll('h1,h2,h3,[role="heading"]')].filter(visible).map(node => node.innerText.trim()), details: [...document.querySelectorAll('details')].map(node => ({ text: node.textContent, open: node.open, visible: visible(node), rect: rect(node) })), geometry: { width: innerWidth, height: innerHeight, devicePixelRatio, locale: navigator.language, theme: document.documentElement.dataset.theme, zoom: getComputedStyle(document.documentElement).zoom, bodyScrollTop: document.body.scrollTop, rootScrollTop: document.documentElement.scrollTop, rootScrollWidth: document.documentElement.scrollWidth, rootClientWidth: document.documentElement.clientWidth } }, facts: window.bindingHarness.facts(), serialized: window.bindingHarness.captureDocument() };
  });
  const {dom,facts}=checkpoint;
  const stepFile = 'step-' + String(sceneStepSequence++).padStart(4, '0');
  const screenshotPath = path.join(output, stepFile + '.png'), domPath = path.join(output, stepFile + '.html');
  await page.screenshot({ path: screenshotPath, animations: 'disabled' }); await fs.writeFile(domPath, await serializeLiveDOM(page,checkpoint.serialized));
  return { action, dom, facts, ui_actions: structuredClone(activeSceneActions), source_details: await sourceDetailObservation(page, facts), screenshot: await reference(screenshotPath), html: await reference(domPath) };
}

async function sceneSettle(page) {
  await page.waitForFunction(() => { const events = window.bindingHarness.facts().events; const ended = new Set(events.filter(row => ['port_return','port_error'].includes(row.kind)).map(row => row.id)); return events.filter(row => row.kind === 'port_call').every(row => ended.has(row.id)); }, undefined, { timeout: 3000 });
  await page.waitForTimeout(40);
}
async function scenePress(page, expression) {
  const dialogs = page.getByRole('dialog').filter({ visible: true }), scope = await dialogs.count() ? dialogs.last() : page;
  const buttons = scope.getByRole('button', { name: expression }).filter({ visible: true });
  const action = { expression: expression.source, flags: expression.flags, matching_count: await buttons.count(), enabled_count: 0, clicked: false, label: null };
  for (const button of await buttons.all()) if (await button.isEnabled()) {
    action.enabled_count++; action.label = await button.textContent(); await button.click(); action.clicked = true;
    activeSceneActions.push(action); await sceneSettle(page); return true;
  }
  activeSceneActions.push(action); return false;
}
async function sceneDeliver(page, stimulus) {
  const hadProgress=await page.locator('[aria-label="自动接入阶段"]').filter({visible:true}).count();
  await page.evaluate(value => window.bindingHarness.deliver(value), stimulus);
  if(hadProgress){
    const deliveryId=(await rawFacts(page)).rpcMessages.filter(row=>row.action==='deliver').at(-1)?.id;
    let observed=false;
    try{
      await page.waitForFunction(actualProgressReadReady,{deliveryId},{timeout:1500});observed=true;
    }catch(error){if(error.name!=='TimeoutError')throw error;
      infrastructureErrors.push({stage:'actual-progress-observation-timeout',stimulus,delivery_id:deliveryId});}
    activeSceneActions.push({kind:'actual-progress-observation-wait',delivery_id:deliveryId,completed:observed});
  }
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function runScene(scene) {
  const requiredMethods = ['snapshot','targets','reviewAssociation','previewCapability','startWindowPreview','controlWindowPreview'];
  if (!['library-window','library-process','first-confirmation','tcc','reuse','cancel-candidate','switch-cancel','idle-refresh-details','picker-cancel','picker-timeout','layout-normal','layout-narrow','layout-200','other-bottle-warning','frame-state','revoke','late-metadata'].includes(scene.name)) requiredMethods.push('associate','associationProgress');
  if (['empty','draft-later','commit-set','commit-failure','owned-remember','unowned-remember','temporary-saved','requires-review','details','effects-order','effects-retry','stale-effects','submit-retry','source-expired','source-query-retry'].includes(scene.name)) requiredMethods.push('sourceSelection','beginReading');
  if (['picker-cancel','picker-timeout','pairing-transfer'].includes(scene.name)) requiredMethods.push('selectWindowPreview');
  if (['pairing-transfer','effects-order','effects-retry','stale-effects'].includes(scene.name)) requiredMethods.push('previewSession','openWindow');
  const missingMethods = requiredMethods.filter(method => !rpcReady?.capabilities.methods.includes(method));
  const rpcCapable = !!rpcReady && rpcReady.capabilities.scene_ids.includes(scene.id) && rpcReady.capabilities.controls.includes('reset') && rpcReady.capabilities.controls.includes('deliver') && !missingMethods.length;
  const fallback = ['library-window','library-process','first-confirmation','idle-refresh-details','reuse','cancel-candidate','empty','draft-later','owned-remember','layout-normal','layout-narrow','layout-200'].includes(scene.name);
  const blocked = [];
  if (!rpcCapable) blocked.push('actual-production-rpc-input:' + scene.id, ...missingMethods.map(method => 'actual-production-rpc-method:' + method));
  if (!rpcCapable && (!fallback || transport?.mode === 'ndjson')) {
    const observations = { steps: [], transport_mode: 'unavailable', blocked_obligations: blocked };
    const trace = await json(scene.id.toLowerCase() + '.trace.json', { schema_version: 1, kind: 'binding-journey-scene-observations/v1', nonce, scene_id: scene.id, journey_id: scene.journey_id, observations, events: [] });
    sceneResults.push({ ...scene, status: 'blocked', blocked_obligations: blocked, assertions: [], observations, trace, screenshot: null, dom: null }); return;
  }
  const viewport = scene.name === 'layout-narrow' ? { width: 760, height: 560 } : { width: 1040, height: 720 };
  const zoom = scene.name === 'layout-200' ? 2 : 1, steps = []; activeSceneActions = [];
  const page = await openPage(['reuse','switch-cancel','cancel-candidate'].includes(scene.name) ? 'same' : 'empty', viewport, zoom, rpcCapable ? scene.id : null);
  const capture = async action => { steps.push(await sceneSnapshot(page, action)); };
  try {
    await capture('library-entry');
    let dialog;
    if (['library-global-owner','scoped-reconnect-owner'].includes(scene.name)) {
      const facts=await rawFacts(page),games=facts.snapshot.games;
      assert.ok(games.length>=2,'Actual two-game seed is required');
      const cards=page.getByRole('button').filter({has:page.locator('[data-cover-state]')});
      assert.equal(await cards.count(),games.length);await cards.first().click();await capture('library-A-selected');
      if(scene.name==='scoped-reconnect-owner'){
        await scenePress(page,/^当前会话$/);await sceneDeliver(page,'scoped_session_disconnected');
        await scenePress(page,/^重新连接游戏…$/);
      }else await scenePress(page,/^连接运行中的游戏…$/);
      dialog=page.getByRole('dialog').filter({visible:true});
      const targetReply=(await rawFacts(page)).rpcMessages.filter(row=>row.method==='targets'&&row.outcome==='ok').at(-1)?.response;
      assert.ok(targetReply?.candidates?.length>=2,'Actual runtime target identities are required');
      const b=games[1].target,candidates=targetReply.candidates.filter(row=>row.environmentKey===b.environmentKey&&row.executablePath===b.canonicalExePath&&row.architecture===b.architecture);
      assert.equal(candidates.length,1,'B runtime identity must independently join its retained game');
      const radios=dialog.getByRole('radio');
      const candidateIndex=targetReply.candidates.findIndex(row=>row.candidateId===candidates[0].candidateId);
      await radios.nth(candidateIndex).check();await sceneSettle(page);await capture('target-reviewed');
      await scenePress(page,/^(关联并接入|切换并接入|复用连接)/);
      await capture(scene.name==='scoped-reconnect-owner'?'scoped-incompatible':'global-associated');
      await sceneDeliver(page,'seal_library_database');await capture('library-database-sealed');
    } else if (scene.name === 'library-process') {
      await scenePress(page, /^连接运行中的游戏…$/); await scenePress(page, /^进程列表$/);
      dialog = page.getByRole('dialog').first(); await dialog.getByRole('radio').first().check(); await page.waitForTimeout(80);
    } else dialog = await openTarget(page);
    await capture('target-reviewed');
    if (['library-global-owner','scoped-reconnect-owner'].includes(scene.name)) { /* completed above from actual runtime identities */ }
    else if (['library-window','library-process','first-confirmation','tcc','reuse'].includes(scene.name)) await capture('review-ready');
    else if (scene.name === 'idle-refresh-details') { await scenePress(page, /^刷新$/); await scenePress(page, /^预览详情$/); await page.keyboard.press('Escape'); await page.waitForTimeout(100); await capture('ordinary-actions'); }
    else if (['picker-cancel','picker-timeout','pairing-transfer'].includes(scene.name)) {
      await scenePress(page, /^(选择对应的游戏窗口…|重新选择…)$/); await page.waitForTimeout(80); await capture(scene.name === 'pairing-transfer' ? 'candidate-shared' : 'picker-terminal');
      if (scene.name === 'pairing-transfer') { await scenePress(page, /^(关联并接入|切换并接入|复用连接)/); await sceneDeliver(page, 'session_frame'); await scenePress(page, /^稍后选择$/); await scenePress(page, /^当前会话$/); await capture('session-frame'); await sceneDeliver(page, 'late_candidate_frame'); await capture('late-candidate-frame'); }
      else { await scenePress(page, /^刷新$/); await page.waitForTimeout(120); await capture('terminal-rechecked'); }
    } else if (['switch-cancel','cancel-candidate'].includes(scene.name)) { await scenePress(page, /^取消$/); await capture('candidate-cancelled'); }
    else if (scene.name === 'stale-review') { await sceneDeliver(page, 'session_replaced'); await capture('session-replaced'); await scenePress(page, /^(关联并接入|切换并接入|复用连接)/); await capture('stale-review-attempt'); }
    else if (['real-stage-order','stale-events','close-operation'].includes(scene.name)) {
      await scenePress(page, /^(关联并接入|切换并接入|复用连接)/); await capture('association-returned');
      if (scene.name === 'real-stage-order') for (const stimulus of ['verified','resources_ready','start_requested','handshake','waiting_text','candidates_ready']) { await sceneDeliver(page, stimulus); await capture('stage-' + stimulus); }
      if (scene.name === 'stale-events') { await sceneDeliver(page, 'current_operation'); await capture('current-operation'); await sceneDeliver(page, 'old_operation_event'); await capture('old-operation-event'); }
      if (scene.name === 'close-operation') { await capture('before-intent'); await page.keyboard.press('Escape'); await sceneDeliver(page, 'operation_continues'); await capture('observer-closed'); }
    } else if (scene.name.startsWith('layout-') || scene.name === 'other-bottle-warning') {
      const identity = dialog.getByText('目标完整路径', { exact: true }); if (await identity.count()) { await identity.first().scrollIntoViewIfNeeded(); await capture('identity-reachable'); }
      const preview = dialog.getByRole('region', { name: '运行窗口预览' }); if (await preview.count()) { await preview.scrollIntoViewIfNeeded(); await capture('preview-reachable'); }
      const confirm = dialog.getByRole('button', { name: /^(关联并接入|切换并接入|复用连接)/ }); if (await confirm.count()) { await confirm.first().scrollIntoViewIfNeeded(); await capture('confirmation-reachable'); }
    } else if (['frame-state','revoke','late-metadata'].includes(scene.name)) {
      await sceneDeliver(page, 'valid_frame'); await capture('valid-frame');
      if (scene.name === 'revoke') { await sceneDeliver(page, 'permission_revoked'); await capture('revoked'); }
      else if (scene.name === 'late-metadata') { await sceneDeliver(page, 'capture_failed'); await sceneDeliver(page, 'metadata_only'); await sceneDeliver(page, 'old_frame'); await capture('late-metadata'); }
      else { await scenePress(page, /^暂停$/); await capture('paused'); await scenePress(page, /^继续$/); await capture('resuming'); await sceneDeliver(page, 'new_frame'); await capture('new-frame'); }
    } else if (['association-retry','source-query-retry'].includes(scene.name)) { await scenePress(page, /^(关联并接入|切换并接入|复用连接)/); await page.waitForTimeout(80); await capture('operation-failed'); await scenePress(page, /重试/); await capture('retry-finished'); }
    else {
      await scenePress(page, /^(关联并接入|切换并接入|复用连接)/); await page.waitForTimeout(80); await capture('before-intent');
      if (scene.name === 'details') { for (const summary of await dialog.locator('summary').all()) if (!await summary.evaluate(node => node.parentElement?.open)) await summary.click(); await capture('details-expanded'); }
      else if (['empty','unowned-remember','requires-review'].includes(scene.name)) await capture('selection-ready');
      else {
        const sourceFacts=await rawFacts(page),typedSourceReply=sourceFacts.rpcMessages.filter(row=>row.method==='sourceSelection'&&row.outcome==='ok').at(-1)?.response;
        const sourceIds=(typedSourceReply?.sources??sourceFacts.session?.sources??[]).map(source=>source.sourceId);
        let chosen=0;
        for(const sourceId of sourceIds.slice(0,2)){
          const escaped=await page.evaluate(value=>CSS.escape(value),sourceId);
          const row=dialog.locator(`[data-source-id="${escaped}"]`);
          assert.equal(await row.count(),1,'Each sourceId must own exactly one row');
          const checkbox=row.getByRole('checkbox').filter({visible:true});assert.equal(await checkbox.count(),1);
          if(await checkbox.isEnabled()){await checkbox.check();chosen++;}
        }
        if (scene.name === 'owned-remember') { const remember = dialog.getByRole('checkbox', { name: /记住/ }); if (await remember.count() === 1 && await remember.isEnabled()) await remember.check(); }
        await capture('draft-selected');
        if (scene.name === 'draft-later') { await scenePress(page, /^稍后选择$/); await capture('later-chosen'); }
        else {
          if (scene.name === 'source-expired') await sceneDeliver(page, 'source_expired');
          await scenePress(page, /^开始阅读$/); await page.waitForTimeout(80); await capture(['commit-failure','submit-retry','source-expired'].includes(scene.name) ? 'submit-failed' : scene.name === 'effects-retry' ? 'effect-failed' : 'submitted');
          if (scene.name === 'submit-retry') { await scenePress(page, /^开始阅读$|重试提交/); await capture('submit-retried'); }
          if (scene.name === 'effects-retry') { await scenePress(page, /重试.*窗口|重试.*浮窗/); await capture('effects-retried'); }
          if (scene.name === 'stale-effects') { await sceneDeliver(page, 'session_replaced'); await capture('session-replaced'); await sceneDeliver(page, 'old_window_effect'); await capture('old-effect-rejected'); }
          if (scene.name === 'temporary-saved') { await sceneDeliver(page, 'reopen_saved_selection'); await scenePress(page, /重新选择文本流/); await capture('saved-selection-reopened'); }
        }
      }
    }
    const inputs = actualSceneInputs(steps);
    if (rpcCapable && ['reuse','switch-cancel','cancel-candidate','revoke'].includes(scene.name) && !inputs.protected_session_id) blocked.push('actual-protected-session-input:' + scene.id);
    if (rpcCapable && scene.name === 'stale-events' && !inputs.current_operation_id) blocked.push('actual-current-operation-input:' + scene.id);
    if (rpcCapable && ['close-operation','stale-events','real-stage-order'].includes(scene.name) && !inputs.committed_operation_ids.length) blocked.push('actual-committed-operation-input:' + scene.id);
    if (rpcCapable && ['stale-review','stale-effects'].includes(scene.name) && !inputs.replacement_session_id) blocked.push('actual-replacement-session-input:' + scene.id);
    if (scene.name === 'details') blocked.push(...steps.at(-1).source_details.blocked_obligations);
    const assertions = sceneChecks(scene, steps), observations = { steps, actual_inputs: inputs, transport_mode: rpcCapable ? 'production_rpc' : 'controlled_diagnostic', blocked_obligations: blocked };
    const stem = scene.id.toLowerCase(), screenshot = path.join(output, stem + '.png'), dom = path.join(output, stem + '.html');
    await page.screenshot({ path: screenshot, animations: 'disabled' }); await fs.writeFile(dom, await serializeLiveDOM(page));
    securityPolicy.violations.push(...await page.evaluate(() => window.bindingCspViolations ?? []));
    const postCheckpointFacts=await rawFacts(page),events=structuredClone(steps.at(-1).facts.events);
    const trace = await json(stem + '.trace.json', { schema_version: 1, kind: 'binding-journey-scene-observations/v1', nonce, scene_id: scene.id, journey_id: scene.journey_id, observations, events,post_checkpoint_facts:postCheckpointFacts });
    const status = assertions.some(row => !row.passed) ? 'failed' : blocked.length ? 'blocked' : 'passed';
    sceneResults.push({ ...scene, status, blocked_obligations: blocked, assertions, observations, trace, screenshot: await reference(screenshot), dom: await reference(dom) }); console.log(scene.id + ': ' + status);
  } finally { await page.close(); }
}

try {
  if (transport?.mode === 'ndjson') { const rpcOutput=path.join(output,'rpc-profiles');await fs.mkdir(rpcOutput,{recursive:false});rpc = new ProductionRpc({ binary: path.resolve(root, transport.binary.path), nonce, sourceTree: transport.source_tree_sha256, cwd: root, output:rpcOutput }); rpcReady = await rpc.start(); }
  const harness = `import React from ${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))};
import {createRoot} from ${JSON.stringify(path.join(desktop, 'node_modules/react-dom/client.js'))};
import {MainWindow} from ${JSON.stringify(path.join(desktop, 'src/windows/main/MainWindow.tsx'))};
import {ApplicationStore} from ${JSON.stringify(path.join(desktop, 'src/bridge/projections/application-store.ts'))};
import {DesignPort} from ${JSON.stringify(path.join(desktop, 'tests/support/design-port.ts'))};
import ${JSON.stringify(path.join(desktop, 'src/ui/reset.css'))};
const captureDocument=${captureLiveDocument.toString()};
const originalProjection=${JSON.stringify(productionValues.selection_required)},runtimeExchanges=${JSON.stringify(transportExchanges)},rpcCapabilities=${JSON.stringify(rpcReady?.capabilities ?? null)};
document.documentElement.dataset.windowRole='main';document.documentElement.dataset.theme='dark';
const port=new DesignPort('empty'),scenario=new URLSearchParams(location.search).get('scenario'),rpcScene=new URLSearchParams(location.search).get('rpcScene');
const game={gameId:'controlled-binding-game',displayName:'受控游戏资料',originalTitle:null,sourceLanguage:'ja',coverAssetRef:null,rowRevision:1,target:{environmentKind:'crossover',environmentKey:'crossover:/Controlled/Bottle',bottleLocator:'/Controlled/Bottle',launchPath:'C:\\\\Controlled\\\\Story.exe',canonicalExePath:'C:\\\\Controlled\\\\Story.exe',targetPathKey:'controlled-target',workingDirectory:null,arguments:[],architecture:'x64',verificationState:'verified'}};
const candidate={candidateId:'controlled-runtime-target',windowTitle:'受控窗口核对样本',executablePath:game.target.canonicalExePath,windowsPid:421,processCreatedU64:'18446744073709551615',architecture:'x64',environmentKey:game.target.environmentKey,bottleLabel:'Controlled',hasWindow:true,windowHandle:'65538',macosHostPid:12345,macosWindowId:23456};
const longSample='受控正文样本：来源与文本必须一致。\\n'.repeat(140)+'完整样本终点';
const sources=[{sourceId:'controlled-source-a',hookName:'受控正文流 A',hookCode:'/HQ@1234:Story.exe',sample:longSample,selected:false,encoding:'UTF-16'},{sourceId:'controlled-source-b',hookName:'受控正文流 B',hookCode:'/HS@4321:Story.exe',sample:'第二条受控来源。',selected:false,encoding:'UTF-16'}];
port.state={...port.state,games:[game],preferences:{...port.state.preferences,theme:'dark'},...(runtimeExchanges?.initial_snapshot??{})};port.targetCandidates=runtimeExchanges?.target_candidates??[candidate];
if(scenario==='same')port.state.session={sessionId:'controlled-existing-session',gameId:game.gameId,sourceKind:'hook',state:'active',paused:false,translationEnabled:true,sources:[]};
const events=[],oldAssociate=port.associate.bind(port),oldDispatch=port.dispatch.bind(port),oldOpen=port.openWindow.bind(port);let transportCalls=[];
port.dispatch=async envelope=>{events.push({kind:'command',envelope:structuredClone(envelope)});return oldDispatch(envelope)};
port.openWindow=async role=>{events.push({kind:'window',role});return oldOpen(role)};
port.startWindowPreview=async request=>{port.previewRequests.push(structuredClone(request));const projection={...structuredClone(originalProjection),consumerId:request.consumerId,leaseId:'controlled-rust-lease-'+port.previewRequests.length,generation:String(++port.previewGeneration)};port.previewProjections.set(projection.leaseId,projection);events.push({kind:'preview_start',request:structuredClone(request),projection:structuredClone(projection),transportRebasedFields:['consumerId','leaseId','generation']});return projection};
port.selectWindowPreview=async request=>{port.previewPickerRequests.push(structuredClone(request));events.push({kind:'explicit_system_picker',request:structuredClone(request)});const projection={...structuredClone(originalProjection),consumerId:request.consumerId,leaseId:'controlled-picked-lease',generation:String(++port.previewGeneration),state:'starting',errorCode:null,detail:'Controlled explicit system-picker response; not Native sharing proof'};port.previewProjections.set(projection.leaseId,projection);return projection};
port.associate=async request=>{events.push({kind:'association',request:structuredClone(request)});const result=await oldAssociate(request);port.publish({session:{sessionId:'controlled-new-session',gameId:request.gameId,sourceKind:'hook',state:'active',paused:false,translationEnabled:true,sources:structuredClone(sources)}});return result};
// Generic production-exchange import, not a business DTO model. Future Rust
// fixture producers own the response fields and effects; absence remains a gap.
if(runtimeExchanges){for(const[method,rows]of Object.entries(runtimeExchanges.methods??{})){let index=0;port[method]=async(...args)=>{const exchange=rows[index++];if(!exchange)throw Error('No production fixture exchange for '+method);const call={method,args:structuredClone(args),exchangeIndex:index-1};transportCalls.push(call);events.push({kind:'runtime_transport_call',...structuredClone(call)});for(const patch of exchange.publications??[])port.publish(patch);if(exchange.error)throw exchange.error;return structuredClone(exchange.response)}}}
const rpcMessages=[],rawPreviewHistory=[],rawPreviewSubscribers=[];let rawPreviewToken=0;
const deliverRawPreview=(record,update)=>{events.push({kind:'preview_callback_delivery',token:record.token,retired:record.retired,update:structuredClone(update)});record.listener(structuredClone(update))};
const forwardRawPreview=update=>{if(rawPreviewHistory.length>=128)throw Error('Raw preview fixture queue exceeded');rawPreviewHistory.push(structuredClone(update));for(const record of rawPreviewSubscribers)deliverRawPreview(record,update)};
const acceptRpc=message=>{rpcMessages.push(structuredClone(message));for(const publication of message.publications){port.state=structuredClone(publication.snapshot);for(const listener of port.listeners)listener(structuredClone(publication))}for(const update of message.previewUpdates)forwardRawPreview(update);for(const event of message.events)events.push({kind:'production_side_effect',requestId:message.id,event:structuredClone(event)});if(message.outcome==='error')throw message.error;return structuredClone(message.response)};
if(rpcScene){port.attachWindowPreview=(projection,listener)=>{const record={token:String(++rawPreviewToken),projection:structuredClone(projection),listener,retired:false};rawPreviewSubscribers.push(record);events.push({kind:'preview_subscription_attach',token:record.token,projection:structuredClone(projection)});queueMicrotask(()=>{for(const update of rawPreviewHistory)deliverRawPreview(record,update)});return()=>{record.retired=true;events.push({kind:'preview_subscription_detach',token:record.token})}};const reset=await window.__lunaBindingRpc({scenario:rpcScene,action:'reset',method:null,args:[]});const initial=acceptRpc(reset);if(!initial?.initial_snapshot)throw Error('Actual Rust reset has no AppSnapshot');port.state=structuredClone(initial.initial_snapshot);for(const method of rpcCapabilities.methods){if(['subscribe','attachWindowPreview'].includes(method))continue;port[method]=async(...args)=>{const response=await window.__lunaBindingRpc({scenario:rpcScene,action:'invoke',method,args});transportCalls.push({method,args:structuredClone(args),responseId:response.id});events.push({kind:'runtime_transport_call',method,args:structuredClone(args),responseId:response.id});return acceptRpc(response)}}}
const instrumented=[...new Set(['snapshot','targets','pickWindow','reviewAssociation','associate','associationProgress','sourceSelection','beginReading','previewCapability','startWindowPreview','selectWindowPreview','controlWindowPreview','previewSession','dispatch','openWindow','closeWindow',...Object.keys(runtimeExchanges?.methods??{}),...(rpcCapabilities?.methods??[])])];
let callSequence=0;for(const method of instrumented){if(['subscribe','attachWindowPreview'].includes(method)||typeof port[method]!=='function')continue;const implementation=port[method].bind(port);port[method]=async(...args)=>{const id=String(++callSequence);events.push({kind:'port_call',id,method,args:structuredClone(args)});try{const response=await implementation(...args);events.push({kind:'port_return',id,method,response:structuredClone(response)});return response}catch(error){events.push({kind:'port_error',id,method,error:typeof error==='object'?{code:error?.code??null,message:error?.message??String(error)}:String(error)});throw error}}}
const store=new ApplicationStore(port);
const calls=()=>events.filter(row=>row.kind==='port_call');
window.bindingHarness={port,store,game,candidate,longSample,sources,events,captureDocument,hold:()=>{port.holdAssociation=true},release:()=>{port.associationResume?.();port.holdAssociation=false},deliver:async stimulus=>{if(!rpcScene)throw Error('A real Rust producer is required for fixture delivery');const response=await window.__lunaBindingRpc({scenario:rpcScene,action:'deliver',method:null,args:[stimulus]});return acceptRpc(response)},facts:()=>({snapshot:structuredClone(store.getSnapshot().snapshot??port.state),session:structuredClone(store.getSnapshot().snapshot?.session??port.state.session),commands:calls().filter(row=>row.method==='dispatch').map(row=>structuredClone(row.args[0])),windows:calls().filter(row=>['openWindow','closeWindow'].includes(row.method)).map(row=>({method:row.method,args:structuredClone(row.args)})),associations:calls().filter(row=>row.method==='associate').map(row=>structuredClone(row.args[0])),previewStarts:calls().filter(row=>row.method==='startWindowPreview').map(row=>structuredClone(row.args[0])),pickers:calls().filter(row=>row.method==='selectWindowPreview').map(row=>structuredClone(row.args[0])),events:structuredClone(events),methodCalls:structuredClone(calls()),transportCalls:structuredClone(transportCalls),rpcMessages:structuredClone(rpcMessages),transportMode:rpcScene?'production_rpc':'controlled_diagnostic'})};
createRoot(document.getElementById('root')).render(<MainWindow store={store}/>);
`;
  await fs.writeFile(path.join(output, 'harness.tsx'), harness);
  await fs.writeFile(path.join(output, 'index.html'), '<!doctype html><html lang="zh-CN"><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><style>html,body,#root{margin:0;width:100%;height:100%;overflow:hidden;background:var(--material-shell-fill)}</style><div id="root"></div><script type="module" src="/harness.tsx"></script></html>');
  const config = path.join(output, 'vite.config.mjs');
  await fs.writeFile(config, `import react from ${JSON.stringify(path.join(desktop, 'node_modules/@vitejs/plugin-react/dist/index.js'))};export default{root:${JSON.stringify(output)},cacheDir:${JSON.stringify(path.join(output, 'vite-cache'))},plugins:[react(),{name:'exact-production-csp',configurePreviewServer(server){server.middlewares.use((_request,response,next)=>{response.setHeader('Content-Security-Policy',${JSON.stringify(productionCsp)});next()})}}],resolve:{alias:[{find:/^react$/,replacement:${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))}},{find:/^react\\/jsx-runtime$/,replacement:${JSON.stringify(path.join(desktop, 'node_modules/react/jsx-runtime.js'))}}]},build:{outDir:${JSON.stringify(path.join(output, 'bundle'))},emptyOutDir:true,target:'es2022'},preview:{host:'127.0.0.1',port:0}};`);
  await execute([process.execPath, path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', config], 'component-build');
  server = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--config', config, '--port', '0'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', data => serverLog += data); server.stderr.on('data', data => serverLog += data);
  for (let n = 0; n < 120; n++) { origin = serverLog.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]; if (origin && (await fetch(origin)).ok) break; if (n === 119 || server.exitCode !== null) throw Error('component server did not become ready: ' + serverLog); await new Promise(resolve => setTimeout(resolve, 100)); }
  const actualHeaders = Object.fromEntries((await fetch(origin)).headers.entries());
  assert.equal(actualHeaders['content-security-policy'], productionCsp);
  responseHeadersRef = await json('component-response-headers.json', { origin, headers: actualHeaders, source_file: securityFile, source_sha256: hash(securityBytes) });
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());

  await check('BJC-01-system-confirmation-state', async () => {
    const page = await openPage();
    try {
      const dialog = await openTarget(page), preview = dialog.getByRole('region', { name: '运行窗口预览', exact: true });
      await preview.locator('[data-preview-state]').count();
      await page.waitForTimeout(40);
      const text = await preview.textContent(), buttons = await preview.getByRole('button').allTextContents();
      const select = preview.getByRole('button', { name: '选择对应的游戏窗口…', exact: true });
      const inputFacts = await rawFacts(page), rpcProjection = inputFacts.events.filter(row => row.kind === 'port_return' && ['startWindowPreview','previewSession','selectWindowPreview'].includes(row.method)).at(-1)?.response;
      const observations = { projection: inputFacts.transportMode === 'production_rpc' ? rpcProjection ?? null : productionValues.selection_required, productionPolicyInput: productionValues.selection_required, transport_mode: inputFacts.transportMode, visibleText: text, buttons, imageCount: await preview.locator('img').count(), timeText: await preview.locator('time').textContent(), selectionButton: { label: await select.count() ? await select.textContent() : null, visible: await select.count() ? await select.isVisible() : false, enabled: await select.count() ? await select.isEnabled() : false } };
      const facts = await rawFacts(page);
      await saveCase(page, 'BJC-01-system-confirmation-state', observations, [assertion('first-system-confirmation-action', true, observations.selectionButton.visible && observations.selectionButton.enabled), assertion('first-system-confirmation-copy', true, /需要.*游戏窗口/.test(text)), assertion('no-fabricated-first-frame', true, observations.imageCount === 0 && observations.timeText.includes('尚无成功画面'))], facts.events);
    } finally { await page.close(); }
  });
  await check('BJC-02-same-target-reuse', async () => {
    const page = await openPage('same');
    try {
      const dialog = await openTarget(page), facts = await rawFacts(page), text = await dialog.textContent();
      const observations = { visibleText: text, buttons: await dialog.getByRole('button').allTextContents(), sameTargetSwitchVisible: await dialog.getByRole('button', { name: '切换并接入', exact: true }).isVisible().catch(() => false), associationCalls: facts.associations, controlledTarget: { gameId: 'controlled-binding-game', candidateId: 'controlled-runtime-target', activeSessionId: 'controlled-existing-session' } };
      await saveCase(page, 'BJC-02-same-target-reuse', observations, [assertion('same-target-not-labelled-switch', false, observations.sameTargetSwitchVisible), assertion('same-target-reuse-explained', true, text.includes('复用'))], facts.events);
    } finally { await page.close(); }
  });
  await check('BJC-03-association-progress', async () => {
    const page = await openPage();
    try {
      const dialog = await openTarget(page);
      await page.evaluate(() => window.bindingHarness.hold());
      await dialog.getByRole('button', { name: '关联并接入', exact: true }).click();
      await page.waitForFunction(() => window.bindingHarness.facts().associations.length === 1);
      await page.evaluate(()=>document.fonts.ready);
      const captures=[];let snapshot=null;
      for(let attempt=1;attempt<=3;attempt++){
        const before=await page.evaluate(capturePendingAssociationState),screenshot=path.join(output,`bjc-03-association-progress.capture-${attempt}.png`);
        await page.screenshot({path:screenshot,animations:'disabled'});
        const after=await page.evaluate(capturePendingAssociationState);
        const record=await json(`bjc-03-association-progress.capture-${attempt}.json`,{schema_version:1,kind:'binding-finite-pending-capture/v1',nonce,case_id:'BJC-03-association-progress',attempt,before:{at_ms:before.at_ms,ui:before.ui,facts:before.facts},after:{at_ms:after.at_ms,ui:after.ui,facts:after.facts},screenshot:await reference(screenshot)});
        captures.push(record);
        if(JSON.stringify(before.ui)===JSON.stringify(after.ui)){snapshot={...before,screenshot,after};break;}
      }
      if(!snapshot)throw Error('Association observation changed during every bounded screenshot; no semantic result inferred');
      const {ui,facts}=snapshot;
      const observations = { visibleText: ui.visibleText, stageLabels:ui.stageLabels, associationCalls: facts.associations,
        pending: ui.dialogVisible && ui.stageLabels.length===3 && facts.associations.length===1 && !ui.duplicateSubmitAvailable,
        duplicateSubmitAvailable:ui.duplicateSubmitAvailable,captureRecords:captures,postCaptureFacts:snapshot.after.facts,
        fixtureStageEvents: [], stageFactCoverage: 'Finite pending presentation only; complete scene separately consumes actual operation facts.' };
      await saveCase(page, 'BJC-03-association-progress', observations, [assertion('three-approved-stages-present', ['窗口已关联', '自动接入', '选择正文'], observations.stageLabels),
        assertion('actual-association-remains-pending',true,observations.pending),assertion('pending-cannot-submit-twice',false,observations.duplicateSubmitAvailable)], facts.events,snapshot);
      await page.evaluate(() => window.bindingHarness.release());
    } finally { await page.close(); }
  });
  await check('BJC-04-selection-draft-confirmation', async () => {
    const page = await openPage();
    try {
      const dialog = await openSources(page), before = await rawFacts(page), begin = dialog.getByRole('button', { name: '开始阅读', exact: true });
      const sourceCheckboxes = [];
      for (const source of before.session.sources) {
        const label = source.hookName || source.sourceId, escaped=await page.evaluate(id=>CSS.escape(id),source.sourceId);
        const row=dialog.locator(`[data-source-id="${escaped}"]`);
        assert.equal(await row.count(),1,'one row per exact sourceId');
        const control = row.getByRole('checkbox');
        sourceCheckboxes.push({ sourceId: source.sourceId, label, matchCount: await control.count(), visible: await control.count() === 1 ? await control.isVisible() : false });
      }
      const sourceCheckboxCount = sourceCheckboxes.filter(row => row.matchCount === 1 && row.visible).length;
      const beginReadingButton = { visible: await begin.count() ? await begin.isVisible() : false, enabled: await begin.count() ? await begin.isEnabled() : false };
      const firstLabel = before.session.sources[0].hookName || before.session.sources[0].sourceId, firstMatcher = new RegExp(firstLabel.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
      if (sourceCheckboxCount) await dialog.getByRole('checkbox', { name: firstMatcher }).check();
      else await dialog.getByRole('button', { name: firstMatcher }).click();
      await page.waitForTimeout(40);
      const after = await rawFacts(page), selectCommands = after.commands.filter(row => ['select_source', 'select_sources', 'begin_reading'].includes(row.command?.kind));
      const draftCommits = facts => facts.methodCalls.filter(row => ['dispatch','beginReading','selectSources','selectSource'].includes(row.method));
      const observations = { sourceCheckboxCount, sourceCheckboxes, beginReadingButton,
        draftClickCommandCount: draftCommits(after).length - draftCommits(before).length,
        draftWindowEffectCount: after.windows.length - before.windows.length,
        draftAssociationCount: after.associations.length - before.associations.length,
        beforeFacts: before, afterFacts: after, selectionCommands: selectCommands, windows: after.windows,
        dialogStillOpen: await page.getByRole('dialog').count() > 0, sourceClickMechanism: sourceCheckboxCount ? 'source-specific-checkbox' : 'production-source-button' };
      await saveCase(page, 'BJC-04-selection-draft-confirmation', observations, [assertion('source-selection-is-checkbox-draft', true, sourceCheckboxCount >= 2), assertion('explicit-begin-reading-present', true, beginReadingButton.visible), assertion('draft-click-does-not-commit', 0, observations.draftClickCommandCount), assertion('draft-click-has-no-window-effects', 0, observations.draftWindowEffectCount), assertion('draft-click-has-no-association-effects', 0, observations.draftAssociationCount), assertion('draft-stays-in-review', true, observations.dialogStillOpen)], after.events);
    } finally { await page.close(); }
  });
  await check('BJC-05-source-details', async () => {
    const page = await openPage();
    try {
      const dialog = await openSources(page);
      for (const summary of await dialog.locator('summary').all()) if (!await summary.evaluate(node => node.parentElement?.open)) await summary.click();
      const text = await dialog.textContent(), facts = await rawFacts(page), sample = facts.session.sources[0].sample;
      const sourceDetails = await dialog.evaluate((node, obligations) => obligations.map(row => {
        const rows = [...node.querySelectorAll('[data-source-id]')].filter(candidate => candidate.dataset.sourceId === row.source_id);
        return { source_id: row.source_id, rowCount: rows.length, fields: row.fields.map(field => {
          const labels = rows.length === 1 ? [...rows[0].querySelectorAll('dt')].filter(term => term.textContent.trim() === field.label) : [];
          const definitions = labels.map(term => term.nextElementSibling).filter(definition => definition?.tagName === 'DD');
          return { ...field, expected: field.value, matchCount: definitions.length, actual: definitions.length === 1 ? definitions[0].textContent.trim() : null };
        }) };
      }), sourceDetailObligations);
      const productionDetailFixtureAvailable = sourceDetailObligations.length > 0;
      const blocked_obligations = productionDetailFixtureAvailable ? [] : ['advanced-source-typed-DTO-disclosure', 'source-u64-typed-DTO-projection'];
      const observations = { expandedText: text, sourceFieldLabels: ['Hook', 'Address', 'Context', 'Context 2'].filter(label => text.includes(label)), sampleText: sample, longSampleRetained: text.includes(sample), productionDetailFixtureAvailable,
        sourceDetails, blocked_obligations, sourceDetailScope: productionDetailFixtureAvailable ? 'Per-source fields scoped by data-source-id and semantic dt/dd, with expected strings bound to actual production response JSON pointers.' : 'Current controlled DTO has no source address/context/subcontext. Typed advanced disclosure and numeric projection remain blocked; no verdict is inferred from candidate identity or unrelated page text.' };
      const assertions = [assertion('full-latest-sample-preserved', true, observations.longSampleRetained)];
      for (const row of sourceDetails) {
        assertions.push(assertion('source-detail-row-' + row.source_id, 1, row.rowCount));
        for (const field of row.fields) assertions.push(assertion('source-detail-' + row.source_id + '-' + field.label, field.expected, field.matchCount === 1 ? field.actual : null));
      }
      await saveCase(page, 'BJC-05-source-details', observations, assertions, facts.events);
    } finally { await page.close(); }
  });
  await check('BJC-06-remember-selection', async () => {
    const page = await openPage();
    try {
      const dialog = await openSources(page), remember = dialog.getByRole('checkbox', { name: /记住选择/ }), facts = await rawFacts(page);
      const observations = { rememberCheckbox: { visible: await remember.count() ? await remember.isVisible() : false, checked: await remember.count() ? await remember.isChecked() : false, enabled: await remember.count() ? await remember.isEnabled() : false }, gameId: facts.session?.gameId, selectionCommands: facts.commands };
      await saveCase(page, 'BJC-06-remember-selection', observations, [assertion('owned-game-remember-choice-present', true, observations.rememberCheckbox.visible && observations.rememberCheckbox.enabled)], facts.events);
    } finally { await page.close(); }
  });
  await check('BJC-07-no-unrequested-picker', async () => {
    const page = await openPage();
    try {
      const dialog = await openTarget(page);
      await page.waitForTimeout(550);
      const before = await rawFacts(page), preview = dialog.getByRole('region', { name: '运行窗口预览', exact: true });
      const select = preview.getByRole('button', { name: /^(选择对应的游戏窗口…|重新选择…)$/ });
      const explicitButtonCount = await select.count();
      const explicitButtonVisible = explicitButtonCount === 1 && await select.isVisible();
      const explicitButtonEnabled = explicitButtonCount === 1 && await select.isEnabled();
      const explicitButton = explicitButtonCount === 1 ? await select.textContent() : null;
      const explicitActionExecuted = explicitButtonVisible && explicitButtonEnabled;
      if (explicitActionExecuted) {
        await select.click();
        // A missing effect is a semantic product failure. The mounted UI and
        // fixture execution remain valid even when the expected call is absent.
        await page.waitForFunction(() => window.bindingHarness.facts().pickers.length > 0, undefined, { timeout: 1000 }).catch(() => {});
      }
      const invoked=(await rawFacts(page)).methodCalls.filter(row=>row.method==='selectWindowPreview').at(-1);
      if(invoked) await page.waitForFunction(id=>window.bindingHarness.facts().events.some(row=>row.id===id&&['port_return','port_error'].includes(row.kind)),invoked.id,{timeout:1500});
      const after = await rawFacts(page), observations = { beforeFacts:before, afterFacts:after, pickerCallsBeforeIntent: before.pickers.length, pickerCallsAfterIntent: after.pickers.length, associationCallsBeforeIntent: before.associations.length, explicitButtonCount, explicitButtonVisible, explicitButtonEnabled, explicitActionExecuted, explicitButton };
      await saveCase(page, 'BJC-07-no-unrequested-picker', observations, [assertion('no-picker-without-explicit-intent', 0, observations.pickerCallsBeforeIntent), assertion('no-association-before-confirmation', 0, observations.associationCallsBeforeIntent), assertion('exactly-one-explicit-picker-action', 1, explicitButtonCount), assertion('explicit-picker-action-available', true, explicitButtonVisible && explicitButtonEnabled), assertion('one-explicit-system-picker', 1, observations.pickerCallsAfterIntent)], after.events);
    } finally { await page.close(); }
  });
  for (const scene of COMPONENT_SCENES) await check(scene.id, () => runScene(scene));
  for (const [variant, viewport, zoom] of [['narrow', { width: 760, height: 560 }, 1], ['css200', { width: 1040, height: 720 }, 2]]) {
    await check('layout-' + variant, async () => {
      const page = await openPage('empty', viewport, zoom);
      try { await openTarget(page); await page.evaluate(() => document.fonts.ready); const file = path.join(output, 'layout-' + variant + '.png'); await page.screenshot({ path: file, animations: 'disabled' }); screenshots.push(await reference(file)); await json('layout-' + variant + '.json', await page.evaluate(() => ({ innerWidth, innerHeight, dpr: devicePixelRatio, zoom: getComputedStyle(document.documentElement).zoom, rootScroll: [scrollY, document.documentElement.scrollTop, document.body.scrollTop], dialog: document.querySelector('[role="dialog"]')?.getBoundingClientRect().toJSON(), scrollAreas: [...document.querySelectorAll('[data-ui-scroll]')].map(node => ({ key: node.dataset.uiScroll, rect: node.getBoundingClientRect().toJSON(), clientHeight: node.clientHeight, scrollHeight: node.scrollHeight })) }))); } finally { await page.close(); }
    });
  }
  await verifyBrowserIdentity(browserIdentity);
} catch (error) {
  infrastructureErrors.push({ stage: 'setup-or-completion', message: error.message, stack: error.stack });
} finally {
  if (browser) await browser.close();
  if (rpc) await rpc.close();
  if (server) { server.kill('SIGTERM'); if (server.exitCode === null) await new Promise(resolve => server.once('close', resolve)); }
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  const sourceAfter = productionIdentity(), stable = JSON.stringify(sourceBefore) === JSON.stringify(sourceAfter);
  if (!stable) infrastructureErrors.push({ stage: 'source-identity', message: 'production source changed during component execution' });
  if (browserErrors.length) infrastructureErrors.push({ stage: 'browser-runtime', errors: browserErrors });
  if (securityPolicy.violations.length) infrastructureErrors.push({ stage: 'production-csp', violations: securityPolicy.violations });
  if (rpc?.protocolErrors.length) infrastructureErrors.push({ stage: 'production-rpc-protocol', errors: rpc.protocolErrors });
  let transportExecution = null;
  if (rpc) {
    const files = {};
    for (const [key, name, raw] of [['stdin','transport-stdin.ndjson',rpc.stdin],['stdout','transport-stdout.log',rpc.stdout],['stderr','transport-stderr.log',rpc.stderr]]) { const file = path.join(output, name); await fs.writeFile(file, raw); files[key] = await reference(file); }
    const transcript = await json('transport-transcript.json', { schema_version: 1, kind: 'binding-journey-runtime-rpc-transcript/v1', nonce, ready: rpc.ready, messages: rpc.transcript });
    transportExecution = await json('transport.execution.json', { command: rpc.command, cwd: '.', environment: rpc.environment, pid: rpc.child?.pid ?? null, started_at: rpc.startedAt, finished_at: rpc.finishedAt, exit_code: rpc.exitCode, signal: rpc.exitSignal, ...files, transcript, protocol_errors: rpc.protocolErrors, ready_received: !!rpc.ready });
    if (rpc.ready && rpc.exitCode !== 0) infrastructureErrors.push({ stage: 'production-rpc-exit', exit_code: rpc.exitCode, signal: rpc.exitSignal });
  }
  const bundleAssets = [];
  async function collectBundle(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name); assert.equal(entry.isSymbolicLink(), false, 'bundle symlinks forbidden');
      if (entry.isDirectory()) await collectBundle(file); else if (entry.isFile()) bundleAssets.push(await reference(file));
    }
  }
  try { await collectBundle(path.join(output, 'bundle')); } catch (error) { infrastructureErrors.push({ stage: 'bundle-closure', message: error.message }); }
  bundleAssets.sort((a,b) => a.path.localeCompare(b.path));
  const componentJourneys = [...new Set(COMPONENT_SCENES.map(row => row.journey_id))].map(id => { const rows = sceneResults.filter(row => row.journey_id === id); return { id, layer: 'component', scene_ids: COMPONENT_SCENES.filter(row => row.journey_id === id).map(row => row.id), status: rows.some(row => row.status === 'failed') ? 'failed' : rows.length !== COMPONENT_SCENES.filter(row => row.journey_id === id).length || rows.some(row => row.status === 'blocked') ? 'blocked' : 'passed' }; });
  const hasFailures = infrastructureErrors.length || cases.length !== 7 || cases.some(row => row.status === 'failed') || sceneResults.some(row => row.status === 'failed');
  const sourceDetailDischarge=scopedSourceDischarge(cases.find(row=>row.id==='BJC-05-source-details'),sceneResults.find(row=>row.id==='BJS-10-details'),nonce);
  const status = hasFailures ? 'failed' : cases.some(row => row.status === 'blocked'&&!(row.id==='BJC-05-source-details'&&sourceDetailDischarge)) || sceneResults.length !== COMPONENT_SCENES.length || sceneResults.some(row => row.status === 'blocked') ? 'blocked' : 'passed';
  const consoleArtifact = await json('browser-console.json', browserConsole), pageErrorArtifact = await json('browser-pageerrors.json', pageErrors);
  const result = { schema_version: 1, kind: 'binding-journey-component-execution/v1', nonce,
    command: [process.execPath, ...process.argv.slice(1)], started_at: started, finished_at: now(), status,
    source_before: sourceBefore, source_after: sourceAfter, source_stable: stable,
    infrastructure_errors: infrastructureErrors, browser_errors: browserErrors, browser_identity: browserIdentity ?? null,
    production_security_policy: securityPolicy, response_headers: responseHeadersRef, console: consoleArtifact, page_errors: pageErrorArtifact,
    runner: await reference(fileURLToPath(import.meta.url)), manifest: producerManifest, projection_values: await reference(projectionPath),
    plan: await reference(path.join(root, 'docs/development/binding-journey-plan.json')), runtime_transport: transport,
    component_mount_succeeded: mountedPageCount > 0, mounted_page_count: mountedPageCount, cases, scenes: sceneResults, component_journeys: componentJourneys, screenshots, executions: commandExecutions,
    bundle_assets: bundleAssets, decoded_images: [...decodedImageEvidence.values()],
    transport_execution: transportExecution, transport_ready: rpcReady, component_scene_registry: COMPONENT_SCENES,source_detail_discharge:sourceDetailDischarge,
    delegated_layers: { 'BJ-14': ['design-independent-state-font-scale-reference', 'native'], 'BJ-13': ['native-os-window-effects'] },
    coverage: { component_scope: 'fourteen-journey-component-layer-only; diagnostic BJC passes never clear another layer', executed_layer: rpcReady ? 'real_react_production_rpc' : 'real_react_controlled_transport', executed_case_ids: cases.map(row => row.id),
      no_global_BJ01_18_completion_claim: true, production_rust_preview_serialization: true,
      remaining_layers: ['domain-relation-and-progress', 'production-SQLite-selection-sets', 'packaged_native', 'same-package-four-games', 'live-translation-service'],
      blocked_obligations: sceneResults.find(row => row.name === 'details')?.observations.steps.at(-1)?.source_details.blocked_obligations ?? ['advanced-source-typed-DTO-disclosure', 'source-u64-typed-DTO-projection'],
      remaining_status: 'blocked_pending_real_layer_execution' },
    native_executed: false, product_acceptance_promoted: false, next_stage_allowed: false };
  await json('verification.json', result);
  console.log(JSON.stringify({ status, cases: cases.length, semantic_failures: cases.filter(row => row.status === 'failed').map(row => row.id), scene_semantic_failures: sceneResults.filter(row => row.status === 'failed').map(row => row.id), blocked_scenes: sceneResults.filter(row => row.status === 'blocked').length, infrastructure_errors: infrastructureErrors.length, output }, null, 2));
  process.exitCode = infrastructureErrors.length ? 2 : status === 'failed' ? 1 : status === 'blocked' ? 3 : 0;
}

}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();

// Independent Native observation consumer. These rules consume original
// journals/SQLite/OS facts, never Python verdicts or production passed flags.
const nativeRegistry = {
  'BJ-01':['before','selected','associated'],'BJ-02':['confirmation'],'BJ-03':['candidate','associated'],
  'BJ-04':['before','after','cancel_intent','cancelled','timeout_intent','timeout'],'BJ-05':['before','reviewed','after'],
  'BJ-06':['candidate'],'BJ-07':['before','closed','cancelled'],'BJ-08':['before','draft','committed'],
  'BJ-09':['before','temporary','save_intent','saved','saved_reopened','unowned'],'BJ-13':['before','completed'],
  'BJ-14':['dialog_default','dialog_narrow'],'BJ-15':['before','new_frame','failed','revoked'],
};
const nativeAmendments = new Set(['BJ-06','BJ-08','BJ-09','BJ-13']);
// A fixed read-only instrument. Its original stdout is consumed independently
// by both inspectors; it has no route to the application or a business API.
const NATIVE_JOURNAL_FD_READER = String.raw`import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const started=Date.now(),file=process.argv[1],limit=64n*1024n*1024n;
if(!path.isAbsolute(file)||path.resolve(file)!==file)throw Error('canonical journal path required');
let component=path.parse(file).root;
for(const part of file.slice(component.length).split(path.sep)){component=path.join(component,part);if(fs.lstatSync(component).isSymbolicLink())throw Error('journal path contains symlink');}
const fd=fs.openSync(file,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);
const facts=s=>({dev:String(s.dev),ino:String(s.ino),size:String(s.size),mtime_ns:String(s.mtimeNs),ctime_ns:String(s.ctimeNs)});
let before,after,bytes;
try{const a=fs.fstatSync(fd,{bigint:true});if(!a.isFile()||a.size<=0n||a.size>limit)throw Error('bounded regular journal required');before=facts(a);bytes=fs.readFileSync(fd);after=facts(fs.fstatSync(fd,{bigint:true}));if(bytes.length>Number(limit)||String(bytes.length)!==before.size||JSON.stringify(before)!==JSON.stringify(after))throw Error('journal changed while reading');}finally{fs.closeSync(fd);}
const executable=fs.realpathSync(process.execPath),executableSha=createHash('sha256').update(fs.readFileSync(executable)).digest('hex');
const result={kind:'native_app_journal_fd_archive/v1',schema_version:1,pid:process.pid,executable_path:executable,executable_sha256:executableSha,observed_started_at_ms:started,observed_finished_at_ms:Date.now(),path:file,before,after,bytes_base64:bytes.toString('base64'),sha256:createHash('sha256').update(bytes).digest('hex')};
process.stdout.write(JSON.stringify(result)+'\n');`;
export function nativeJournalReaderSource() { return NATIVE_JOURNAL_FD_READER; }
export function nativeJournalReadCommand(journalPath) {
  nvNeed(typeof journalPath==='string'&&path.isAbsolute(journalPath),'absolute Native journal path required');
  return [process.execPath,'--input-type=module','--eval',NATIVE_JOURNAL_FD_READER,journalPath];
}
function nvNeed(value, message) { if (!value) throw Error(message); }
function nvStable(value) {
  if (Array.isArray(value)) return value.map(nvStable);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(k=>[k,nvStable(value[k])]));
  return value;
}
function nvEqual(a,b) { return JSON.stringify(nvStable(a))===JSON.stringify(nvStable(b)); }
function nvObject(value,message='Native typed object missing') { nvNeed(value && typeof value==='object' && !Array.isArray(value),message);return value; }
function nvDecimal(value) { return typeof value==='string' && /^(0|[1-9][0-9]*)$/.test(value) && BigInt(value)<=18446744073709551615n; }
function nvI64(value) {
  nvNeed(typeof value==='string'&&/^(0|[1-9][0-9]*)$/.test(value)||Number.isSafeInteger(value)&&value>=0,'Native SQLite integer was rounded/noncanonical');
  const n=BigInt(value);nvNeed(n>=0n&&n<=9223372036854775807n,'Native SQLite integer outside i64');return n;
}
function nvU32(value) {const n=nvI64(value);nvNeed(n>0n&&n<=4294967295n,'Native DTO u32 generation/revision outside lossless bounds');return Number(n);}
function nvHead(step) {
  const rows=step.tables.source_selection_sets.filter(r=>r.session_id===step.context.session_id);if(!rows.length)return null;
  nvNeed(rows.every(r=>r.state==='sealed'),'Native snapshot contains unsealed current selection');
  const versions=rows.map(r=>nvI64(r.selection_set_revision));nvNeed(versions.every(n=>n>0n)&&new Set(versions.map(String)).size===rows.length,'Native selection head malformed/duplicated');
  return rows[versions.reduce((best,n,k)=>n>versions[best]?k:best,0)];
}
function nvSampling(time,intervals,events,context,fence) {
  nvNeed(nvEqual(Object.keys(intervals).sort(),['database','lsof','os']),'Native actual sampling intervals missing');
  for(const p of Object.values(intervals))nvNeed(p.length===2&&p.every(n=>Number.isSafeInteger(n)&&n>0)&&p[0]<=p[1],'Native sampling interval malformed');
  nvNeed(intervals.os[0]<=time&&time<=intervals.os[1],'Native OS outside actual observer command');
  const start=Math.min(...Object.values(intervals).map(p=>p[0])),finish=Math.max(...Object.values(intervals).map(p=>p[1]));
  const coverage=nvJournalFence(finish,events,context,fence),coveredUntil=coverage.finished_at_ms;
  const readonly=new Set(['snapshot','resources','targets','associationProgress','sourceSelection','previewSession','gameWindowBinding','windowCapabilities','readGameCover']);
  const identity=e=>['session_id','operation_id','selection_set_revision','active_target_identity_sha256'].map(k=>e[k]??null),pending=new Map();
  for(const e of events){if(start<=e.observed_at_ms&&e.observed_at_ms<=coveredUntil)nvNeed(!['persist_commit','show_hud_completed','hide_main_completed','window_effect_failed','operation_cancelled'].includes(e.action)&&!(['port_call','port_return'].includes(e.action)&&!readonly.has(e.method))&&!(e.action==='context_transition'&&!nvEqual(identity(e),identity(context))),'Native sampling crossed authority mutation');
    const key=JSON.stringify([e.correlation_id,e.method]);if(e.action==='port_call'&&!readonly.has(e.method))pending.set(key,e);else if(e.action==='port_return'){const a=pending.get(key);pending.delete(key);nvNeed(!a||!(a.observed_at_ms<start&&e.observed_at_ms>=start),'Native sampling crossed in-flight mutation');}}
  nvNeed(![...pending.values()].some(e=>e.observed_at_ms<=coveredUntil),'Native mutating method interval not closed');return {started_at_ms:start,finished_at_ms:finish,intervals,journal_fence:coverage};
}
function nvExactDecimal(value) {
  nvNeed(typeof value==='string'&&/^(0|[1-9][0-9]*)$/.test(value),'Native FD integer must be canonical decimal text');return BigInt(value);
}
function nvBase64(value) {
  nvNeed(typeof value==='string','Native original base64 bytes absent');const bytes=Buffer.from(value,'base64');
  nvNeed(bytes.toString('base64')===value,'Native original base64 bytes noncanonical');return bytes;
}
function nvRawLines(bytes) {
  nvNeed(bytes.length>0&&bytes.length<=64*1024*1024&&bytes.at(-1)===10,'Native original journal incomplete or unbounded');
  const lines=[];let start=0;
  for(let end=bytes.indexOf(10,start);end>=0;end=bytes.indexOf(10,start)){nvNeed(end>start,'Native blank journal record');lines.push(bytes.subarray(start,end+1));start=end+1;}
  nvNeed(start===bytes.length,'Native journal partial final record');return lines;
}
function nvArchiveFacts(bytes) {
  const lines=nvRawLines(bytes),rows=lines.map(line=>{const text=line.toString('utf8');nvNeed(Buffer.from(text,'utf8').equals(line),'Native journal invalid UTF-8');return strictProtocolJson(text);});
  nvNeed(rows.length>=3&&rows[0].kind==='binding_journal_begin'&&rows.at(-1).kind==='binding_journal_health'&&rows.filter(r=>r.kind==='binding_journal_begin').length===1,'Native journal header/events/closed footer missing');
  const keys=['nonce','pid','profile_id','boot_sec','boot_usec'],cohort=Object.fromEntries(keys.map(k=>[k,rows[0][k]])),events=[],eventBytes=[];
  for(let k=1;k<rows.length;k++){const row=rows[k];nvNeed(keys.every(name=>row[name]===cohort[name]),'Native journal FD cohort changed');
    if(row.kind==='binding_journal_event'){nvNeed(Number.isSafeInteger(row.sequence)&&row.sequence===events.length+1,'Native journal event prefix discontinuous');events.push(row);eventBytes.push(lines[k]);}
    else {nvNeed(row.kind==='binding_journal_health'&&['accepted','written','last_sequence'].every(name=>Number.isSafeInteger(row[name])&&row[name]===events.length)&&['dropped','invalid','write_failures'].every(name=>row[name]===0),'Native journal footer has loss or undrained records');}}
  nvNeed(events.length,'Native journal event prefix empty');return {header:rows[0],headerBytes:lines[0],events,eventBytes,health:rows.at(-1)};
}
function nvJournalFence(finished,events,context,fence) {
  const keys=['query_correlation_id','cohort','archive','profile','tables','held_before','held_after','reader','fd_interval','archive_interval','archive_pid','executable','writer_interval'];
  nvNeed(nvEqual(Object.keys(nvObject(fence)).sort(),keys.sort()),'Native sampling post-query FD fence absent');
  const {cohort,archive,reader}=fence;
  nvNeed(nvEqual(Object.keys(nvObject(archive)).sort(),['sha256','size','health','records_base64'].sort())&&nvEqual(Object.keys(nvObject(reader)).sort(),['kind','schema_version','pid','executable_path','executable_sha256','observed_started_at_ms','observed_finished_at_ms','path','before','after','bytes_base64','sha256'].sort()),'Native actual archive/reader fields differ from fixed instrument');
  nvNeed(nvEqual(Object.keys(nvObject(cohort)).sort(),['nonce','pid','profile_id','boot_sec','boot_usec'].sort())&&Number.isSafeInteger(cohort.pid)&&cohort.pid>0&&typeof cohort.nonce==='string'&&/^[0-9a-f]{32}$/.test(cohort.nonce),'Native read-fence cohort malformed');
  for(const key of ['fd_interval','archive_interval','writer_interval']){const pair=fence[key];nvNeed(Array.isArray(pair)&&pair.length===2&&pair.every(n=>Number.isSafeInteger(n)&&n>0)&&pair[0]<=pair[1],'Native read-fence actual command interval invalid');}
  const [fdStart,fdEnd]=fence.fd_interval,[readStart,readEnd]=fence.archive_interval,[writerStart,writerEnd]=fence.writer_interval;
  nvNeed(writerStart<=finished&&finished<fdStart&&fdStart<=fdEnd&&fdEnd<=readStart&&readStart<=readEnd&&readEnd<=writerEnd,'Native post-sample read commands escaped writer interval');
  nvNeed(reader?.kind==='native_app_journal_fd_archive/v1'&&reader.schema_version===1&&Number.isSafeInteger(fence.archive_pid)&&fence.archive_pid>0&&reader.pid===fence.archive_pid&&reader.executable_path===fence.executable?.path&&reader.executable_sha256===fence.executable?.sha256&&/^[0-9a-f]{64}$/.test(reader.executable_sha256)&&path.isAbsolute(reader.executable_path),'Native journal archive reader PID/executable differs');
  nvNeed(Number.isSafeInteger(reader.observed_started_at_ms)&&Number.isSafeInteger(reader.observed_finished_at_ms)&&readStart<=reader.observed_started_at_ms&&reader.observed_started_at_ms<=reader.observed_finished_at_ms&&reader.observed_finished_at_ms<=readEnd,'Native reader observations outside real process interval');
  const before=nvObject(fence.held_before),after=nvObject(fence.held_after);
  nvNeed(nvEqual(Object.keys(before).sort(),['pid','fd','path','inode'].sort())&&nvEqual(Object.keys(after).sort(),['pid','fd','path','inode','dev'].sort())&&Object.keys(before).every(k=>after[k]===before[k])&&before.pid===cohort.pid&&typeof before.fd==='string'&&/^[0-9]+[a-z]*$/.test(before.fd)&&path.isAbsolute(before.path)&&reader.path===before.path,'Native post-query FD differs from original app-held journal');
  const stat=nvObject(reader.before);nvNeed(nvEqual(Object.keys(stat).sort(),['dev','ino','size','mtime_ns','ctime_ns'].sort())&&nvEqual(reader.after,stat),'Native same-FD journal changed during read');
  Object.values(stat).forEach(nvExactDecimal);nvNeed(stat.ino===before.inode&&stat.dev===after.dev&&nvExactDecimal(stat.size)<=64n*1024n*1024n,'Native original FD device/inode differs');
  const raw=nvBase64(reader.bytes_base64);nvNeed(Array.isArray(archive.records_base64)&&archive.records_base64.length,'Native full raw journal line inventory absent');
  const fullLines=archive.records_base64.map(value=>{const line=nvBase64(value);nvNeed(line.at(-1)===10&&line.indexOf(10)===line.length-1,'Native original journal single LF not retained');return line;}),fullRaw=Buffer.concat(fullLines);
  nvNeed(BigInt(raw.length)===nvExactDecimal(stat.size)&&createHash('sha256').update(raw).digest('hex')===reader.sha256&&BigInt(fullRaw.length)===nvExactDecimal(archive.size)&&createHash('sha256').update(fullRaw).digest('hex')===archive.sha256,'Native original FD/full journal SHA or size differs');
  const prefix=nvArchiveFacts(raw),full=nvArchiveFacts(fullRaw);
  nvNeed(prefix.headerBytes.equals(full.headerBytes)&&nvEqual(prefix.header,full.header)&&Object.keys(cohort).every(k=>full.header[k]===cohort[k])&&nvEqual(full.health,archive.health)&&nvEqual(full.events,events)&&prefix.events.length<=events.length&&prefix.eventBytes.every((line,k)=>line.equals(full.eventBytes[k])),'Native FD read is not exact full-journal event prefix');
  nvNeed(prefix.events.every(e=>Number.isSafeInteger(e.observed_at_ms)&&e.observed_at_ms<=reader.observed_finished_at_ms)&&events.slice(prefix.events.length).every(e=>e.observed_at_ms>reader.observed_finished_at_ms),'Native archive omitted a fact within actual read coverage');
  const correlation=fence.query_correlation_id;nvNeed(typeof correlation==='string'&&correlation,'Native actual query correlation absent');
  const query=prefix.events.filter(e=>e.correlation_id===correlation&&['port_call','port_return'].includes(e.action)),method=context.session_id!==null&&context.session_id!==undefined?'sourceSelection':'snapshot';
  nvNeed(query.length===2&&nvEqual(query.map(e=>e.action),['port_call','port_return'])&&query.every(e=>e.method===method),'Native actual post-sample readonly call/return missing');
  const [call,returned]=query;nvNeed(finished<call.observed_at_ms&&call.observed_at_ms<=returned.observed_at_ms&&returned.observed_at_ms<fdStart,'Native actual query did not fence sampling end');
  const sampled={tables:fence.tables,context,journal:query},profile=nvProfile(sampled);
  nvNeed(nvEqual(fence.profile,profile)&&cohort.profile_id===profile.profileId,'Native readonly query profile differs from SQLite');
  if(method==='sourceSelection'){
    nvNeed(nvEqual(call.args,{sessionId:context.session_id,expectedProfileGeneration:profile.profileGeneration}),'Native readonly source query arguments differ');
    const projection=nvSource(sampled).projection,head=nvHead(sampled);
    if(head){const members=fence.tables.source_selection_members.filter(m=>m.session_id===context.session_id&&nvI64(m.selection_set_revision)===nvI64(head.selection_set_revision)).sort((a,b)=>a.member_ordinal-b.member_ordinal);
      nvNeed(members.length>=1&&members.length<=64&&members.length===head.member_count&&new Set(members.map(m=>m.source_id)).size===members.length&&members.every((m,k)=>m.member_ordinal===k),'Native post-sample current members incomplete');
      const current={sessionId:context.session_id,setRevision:String(nvI64(head.selection_set_revision)),operationId:head.operation_id,persistence:head.persistence_kind,selectedAtMs:String(head.selected_at_ms),members:members.map(m=>({sourceId:m.source_id,selectionRevision:nvU32(m.selection_revision),ordinal:m.member_ordinal}))};
      nvNeed(nvEqual(projection.current,current),'Native post-sample set membership/persistence differs');}
  } else {
    const value=nvObject(returned.response),hooks=fence.tables.sessions.filter(row=>row.source_kind==='hook');
    const latest=hooks.reduce((best,row)=>!best||nvI64(row.started_at_ms)>nvI64(best.started_at_ms)||(nvI64(row.started_at_ms)===nvI64(best.started_at_ms)&&row.session_id>best.session_id)?row:best,null);
    const expected=latest?{sessionId:latest.session_id,gameId:latest.game_id,sourceKind:latest.source_kind,state:latest.state}:null;
    nvNeed(nvEqual(call.args,{})&&Object.keys(profile).every(k=>value[k]===profile[k])&&(expected?value.session&&Object.keys(expected).every(k=>value.session[k]===expected[k]):value.session===null)&&Number.isSafeInteger(value.commitRevision)&&value.commitRevision>=0&&value.commitRevision<=4294967295&&BigInt(value.commitRevision)===nvI64(fence.tables.profile_meta[0].commit_revision),'Native readonly snapshot differs from actual profile/latest Hook session');
  }
  return {query_correlation_id:correlation,query_sequence:returned.sequence,finished_at_ms:reader.observed_finished_at_ms,archive_sha256:reader.sha256,last_sequence:prefix.health.last_sequence,fd:before.fd};
}
export function nativeObservationRole(caseId,stepId) {
  nvNeed((caseId==='BJ-06'?['waiting','candidate']:nativeRegistry[caseId]??[]).includes(stepId),'unregistered Native role');
  return new Set(['BJ-08:committed','BJ-09:temporary','BJ-09:saved','BJ-13:completed']).has(`${caseId}:${stepId}`)?'hud':'main';
}
function nvInventory(caseId,steps) {
  const ids=steps.map(s=>s.id);
  nvNeed(caseId==='BJ-06'?(nvEqual(ids,['candidate'])||nvEqual(ids,['waiting','candidate'])):nvEqual(ids,nativeRegistry[caseId]),'Native fixed step inventory differs');
  for(const s of steps){nativeObservationRole(caseId,s.id);nvNeed(!['role','observation_role','allow_main_absent','milestones','branch'].some(k=>k in s),'artifact supplied Native role/branch');}
}
function nvProfile(step) {
  const rows=step.tables.profile_meta;
  nvNeed(Array.isArray(rows)&&rows.length===1,'actual SQLite profile missing');const row=rows[0];
  nvNeed(row.runtime_state==='active'&&row.profile_id&&row.server_instance_id,'full active profile invalid');
  return {profileId:row.profile_id,serverInstanceId:row.server_instance_id,profileGeneration:nvU32(row.runtime_generation)};
}
function nvPairs(events,method) {
  const open=new Map(),pairs=[];
  for(const e of events){if(e.method!==method)continue;
    if(e.action==='port_call'){nvNeed(!open.has(e.correlation_id),'duplicate actual Native call');open.set(e.correlation_id,e);}
    else if(e.action==='port_return'){const a=open.get(e.correlation_id);nvNeed(a&&a.sequence<e.sequence,'Native return lacks actual call');open.delete(e.correlation_id);pairs.push([a,e]);}}
  return pairs;
}
function nvSelection(step) {
  const c=step.context,p=nvProfile(step),op=c.operation_id,sid=c.session_id,rev=c.selection_set_revision;
  nvNeed(op&&sid&&typeof rev==='string'&&/^[1-9][0-9]*$/.test(rev),'current Native selection identity missing');
  const commits=step.journal.filter(e=>e.action==='persist_commit'&&e.operation_id===op);nvNeed(commits.length===1,'one actual durable commit required');
  const commit=commits[0],receipt=nvObject(commit.response),authority={profile:p,sessionId:sid,setRevision:rev,operationId:op};
  nvNeed(nvEqual(receipt.profile,p)&&nvEqual(receipt.effectAuthority,authority)&&receipt.operationId===op&&receipt.replayed===false,'Native receipt full authority differs');
  const sets=step.tables.source_selection_sets.filter(r=>r.operation_id===op&&r.session_id===sid&&String(r.selection_set_revision)===rev);
  nvNeed(sets.length===1&&sets[0].state==='sealed'&&sets[0].origin_kind==='explicit_set','actual current sealed set missing');const row=sets[0];
  nvNeed(nvEqual(nvHead(step),row),'Native receipt/context is not the current sealed head');
  const members=step.tables.source_selection_members.filter(m=>m.session_id===sid&&String(m.selection_set_revision)===rev).sort((a,b)=>a.member_ordinal-b.member_ordinal);
  nvNeed(members.length>=1&&members.length<=64&&members.length===row.member_count&&new Set(members.map(m=>m.source_id)).size===members.length&&members.every((m,k)=>m.member_ordinal===k),'Native complete members differ');
  const selection={sessionId:sid,setRevision:rev,operationId:op,persistence:row.persistence_kind,selectedAtMs:String(row.selected_at_ms),members:members.map(m=>({sourceId:m.source_id,selectionRevision:nvU32(m.selection_revision),ordinal:m.member_ordinal}))};
  nvNeed(nvEqual(receipt.selection,selection),'Native receipt member/persistence differs');
  const session=step.tables.sessions.filter(r=>r.session_id===sid);nvNeed(session.length===1&&session[0].target_identity_sha256===c.active_target_identity_sha256,'actual selection target differs');
  const pairs=nvPairs(step.journal,'beginReading').filter(([a])=>a.args?.operationId===op);nvNeed(pairs.length===1,'one actual beginReading required');
  const [call,returned]=pairs[0],args=nvObject(call.args);
  nvNeed(args.sessionId===sid&&args.expectedProfileGeneration===p.profileGeneration&&nvEqual(args.sourceIds,members.map(m=>m.source_id))&&typeof args.remember==='boolean'&&args.remember===(row.persistence_kind==='saved_snapshot')&&args.expectedSetRevision===(rev==='1'?null:String(BigInt(rev)-1n))&&call.sequence<commit.sequence&&commit.sequence<returned.sequence&&call.observed_at_ms<=commit.observed_at_ms&&commit.observed_at_ms<=returned.observed_at_ms&&returned.observed_at_ms<=step.os.observed_at_ms&&nvEqual(returned.response?.selection,receipt),'actual selection request/return differs');
  for(const m of members){const sources=step.tables.capture_sources.filter(s=>s.source_id===m.source_id&&s.session_id===sid),old=step.tables.source_selections.filter(s=>s.source_id===m.source_id&&s.session_id===sid&&s.selection_revision===m.selection_revision);nvNeed(sources.length===1&&old.length===1&&old[0].persistence_kind===row.persistence_kind,'actual source lineage missing');}
  return {commit,receipt,authority,row,members,call,returned};
}
function nvSuccess(before,outcome) {
  const proof=nvSelection(outcome),identity=s=>[s.process.pid,s.process.start_sec,s.process.start_usec,s.process.executable,s.os.boot_sec,s.os.boot_usec];
  nvNeed(nvEqual(before.database_owner??null,outcome.database_owner??null),'Native database FD/cohort differs');
  nvNeed(nvEqual(nvProfile(before),nvProfile(outcome))&&before.context.session_id===outcome.context.session_id&&before.context.active_target_identity_sha256===outcome.context.active_target_identity_sha256,'Native before/outcome authority changed');
  nvNeed(nvEqual(identity(before),identity(outcome))&&before.hud?.window_id===outcome.hud?.window_id&&outcome.hud?.pid===outcome.process.pid,'Native OS cohort/HUD differs');
  nvNeed(before.main.on_screen===true&&before.main.window_id===outcome.main.window_id&&outcome.main.on_screen===false&&outcome.hud?.on_screen===true&&outcome.os.observed_at_ms>before.os.observed_at_ms,'actual known Main hidden/HUD visible missing');
  const c=outcome.context,relevant=outcome.journal.filter(e=>e.correlation_id===c.operation_id),effects=relevant.filter(e=>['show_hud_completed','hide_main_completed','window_effect_failed'].includes(e.action));
  const attempts=[[proof.call,proof.returned],...nvPairs(relevant,'retryReadingWindowEffects')];let lastSuccess=null;
  for(const [call,returned] of attempts){
    if(call.method==='retryReadingWindowEffects')nvNeed(nvEqual(call.args,proof.authority),'stale Native retry authority');
    const result=nvObject(call.method==='beginReading'?returned.response?.effects:returned.response);
    nvNeed(nvEqual(result.authority,proof.authority),'Native effect result authority differs');
    const events=effects.filter(e=>call.sequence<e.sequence&&e.sequence<returned.sequence),expected=[];
    const times=[call.observed_at_ms,...events.map(e=>e.observed_at_ms),returned.observed_at_ms];nvNeed(times.every((n,k)=>k===0||times[k-1]<=n)&&returned.observed_at_ms<=outcome.os.observed_at_ms,'Native effect actual times escaped call/return');
    for(const [key,name,success] of [['showHud','show_hud','show_hud_completed'],['hideMain','hide_main','hide_main_completed']]){
      const r=nvObject(result[key]);if(r.state==='succeeded'&&r.error===null)expected.push([success,name,null]);
      else if(r.state==='failed'&&r.error?.code)expected.push(['window_effect_failed',name,r.error.code]);
      else nvNeed(nvEqual(r,{state:'not_attempted',error:null}),'Native effect state malformed');}
    nvNeed(nvEqual(events.map(e=>[e.action,e.effect,e.error_code]),expected),'Native callbacks differ from actual effect return');
    for(const e of events)nvNeed(e.operation_id===c.operation_id&&e.session_id===c.session_id&&e.selection_set_revision===c.selection_set_revision&&e.active_target_identity_sha256===c.active_target_identity_sha256,'Native effect context differs');
    const show=events.filter(e=>e.action==='show_hud_completed'),hide=events.filter(e=>e.action==='hide_main_completed');
    if(hide.length)nvNeed(show.length===1&&proof.commit.sequence<show[0].sequence&&show[0].sequence<hide[0].sequence,'Native commit/show/hide order differs');
    if(nvEqual(result.showHud,{state:'succeeded',error:null})&&nvEqual(result.hideMain,{state:'succeeded',error:null})&&result.retryable===false)lastSuccess=returned.sequence;
    else nvNeed(result.retryable===true,'Native failed effect lost retryable status');}
  nvNeed(lastSuccess===attempts.at(-1)[1].sequence&&effects.every(e=>attempts.some(([a,b])=>a.sequence<e.sequence&&e.sequence<b.sequence)),'manual/later Native hide cannot repair missing authorized effects');
  const mark=before.raw.journal_end_sequence,time=before.os.observed_at_ms,fresh=e=>e.sequence>mark&&e.observed_at_ms>time;
  if(fresh(proof.call)&&fresh(proof.commit)){nvNeed(!before.tables.source_selection_sets.some(r=>r.operation_id===c.operation_id),'fresh Native handoff reused committed operation');const prior=nvHead(before);nvNeed(proof.call.args.expectedSetRevision===(prior?String(nvI64(prior.selection_set_revision)):null),'new Native commit CAS differs from before head');}
  else {const prior=nvSelection(before),original=prior.returned.response?.effects,retries=attempts.filter(([a])=>a.method==='retryReadingWindowEffects'&&fresh(a));
    nvNeed(nvEqual(prior.authority,proof.authority)&&nvEqual(prior.commit,proof.commit)&&original?.retryable===true&&retries.length&&!outcome.journal.some(e=>e.sequence>mark&&e.action==='persist_commit')&&!attempts.some(([a,b])=>a.method==='retryReadingWindowEffects'&&b.sequence<=mark&&b.response?.retryable===false)&&retries.at(-1)[1].sequence===attempts.at(-1)[1].sequence,'historical Native success cannot stand in for fresh commit or failed-effect retry');}
  return proof;
}
function nvSource(step) {
  const profile=nvProfile(step),sid=step.context.session_id,pairs=nvPairs(step.journal,'sourceSelection').filter(([a])=>a.args?.sessionId===sid);
  nvNeed(pairs.length,'actual sourceSelection evidence absent');const [a,b]=pairs.at(-1),p=nvObject(b.response);
  nvNeed(a.args.expectedProfileGeneration===profile.profileGeneration&&nvEqual(p.profile,profile)&&p.sessionId===sid&&Array.isArray(p.sources)&&new Set(p.sources.map(s=>s.sourceId)).size===p.sources.length,'source projection profile/session differs');
  const head=nvHead(step);nvNeed(head?p.current?.sessionId===sid&&p.current?.setRevision===String(nvI64(head.selection_set_revision))&&p.current?.operationId===head.operation_id:p.current===null,'source projection borrowed noncurrent head');
  const fields={streamIdentitySha256:'stream_identity_sha256',hookName:'hook_name',hookCode:'hook_code',encoding:'encoding',address:'hook_address_u64',context:'context_u64',context2:'subcontext_u64',identityExtensionsJson:'identity_extensions_json'};
  for(const s of p.sources){const rows=step.tables.capture_sources.filter(r=>r.source_id===s.sourceId&&r.session_id===sid);nvNeed(rows.length===1&&Object.entries(fields).every(([k,v])=>s[k]===rows[0][v])&&['address','context','context2'].every(k=>nvDecimal(s[k])),'source actual typed identity differs');}
  return {call:a,returned:b,projection:p};
}
function nvSaved(step,row) {
  const value=JSON.parse(row.match_snapshot_json),members=step.tables.source_selection_members.filter(m=>m.session_id===row.session_id&&m.selection_set_revision===row.selection_set_revision).sort((a,b)=>a.member_ordinal-b.member_ordinal);
  const sources=members.map(m=>{const rows=step.tables.capture_sources.filter(s=>s.session_id===row.session_id&&s.source_id===m.source_id);nvNeed(rows.length===1,'saved source not unique');const s=rows[0];nvNeed(['hook_address_u64','context_u64','subcontext_u64'].every(k=>nvDecimal(s[k])),'saved u64 identity differs');return {sourceId:s.source_id,streamIdentitySha256:s.stream_identity_sha256,hookName:s.hook_name,hookCode:s.hook_code,encoding:s.encoding,address:s.hook_address_u64,context:s.context_u64,context2:s.subcontext_u64,identityExtensionsJson:s.identity_extensions_json};});
  nvNeed(value.schemaVersion===1&&value.matchCapability==='requires_review'&&value.targetIdentitySha256===step.context.active_target_identity_sha256&&sources.length>0&&sources.length===row.member_count&&nvEqual(value.sources,sources),'actual saved descriptor differs');
}
function nvReopen(before,after) {
  nvNeed(nvEqual(before.database_owner??null,after.database_owner??null),'reopen database FD/cohort differs');
  nvNeed(nvEqual(nvProfile(before),nvProfile(after))&&nvEqual(before.context,after.context),'reopen full authority changed');
  for(const k of ['sessions','source_selection_sets','source_selection_members','source_selections'])nvNeed(nvEqual(before.tables[k],after.tables[k]),'reopen changed durable selection');
  const actions=after.native_actions??[];nvNeed(nvEqual(actions.map(a=>a.id),['normal_show_main','open_current_sources']),'actual reopen actions absent');let cursor=before.os.observed_at_ms;
  for(const a of actions){nvNeed(cursor<=a.started_at_ms&&a.started_at_ms<=a.finished_at_ms&&a.finished_at_ms<=after.os.observed_at_ms,'reopen command interval differs');cursor=a.finished_at_ms;}
  nvNeed(after.main.on_screen===true&&before.main.window_id===after.main.window_id,'normal reopen Main differs');
  const old=before.raw.journal_end_sequence,delta=after.journal.filter(e=>e.sequence>old);
  nvNeed(!delta.some(e=>['persist_commit','show_hud_completed','hide_main_completed'].includes(e.action)||['associate','beginReading','retryReadingWindowEffects','disconnect'].includes(e.method)),'reopen repeated original effects/association/commit');
  const p=nvSource(after);nvNeed(p.call.sequence>old,'reopened sources borrowed cached query');return p.projection;
}
function nvProgress(steps) {
  const by=new Map(steps.map(s=>[s.id,s])),candidate=by.get('candidate');nvNeed(candidate,'actual candidate panel absent');
  const profile=nvProfile(candidate),sid=candidate.context.session_id;
  const qualified=nvPairs(candidate.journal,'associationProgress').map(([a,b])=>[a,b,b.response]).filter(([a,b,v])=>v&&v.sessionId===sid&&nvEqual(v.profile,profile)&&v.superseded===false&&v.error===null&&a.args?.operationId===v.operationId&&a.args.expectedProfileGeneration===profile.profileGeneration);
  nvNeed(qualified.length&&qualified.at(-1)[2].stage==='candidates_ready','actual current Native progress missing');const [call,returned,p]=qualified.at(-1),op=p.operationId;
  nvNeed(candidate.tables.capture_operations.some(r=>r.operation_id===op&&r.session_id===sid),'actual operation differs');
  const ledger=new Map(candidate.tables.hook_events.filter(r=>r.operation_id===op&&r.session_id===sid).map(r=>[r.event_id,r]));
  nvNeed(Array.isArray(p.facts)&&p.facts.length&&new Set(p.facts.map(f=>f.eventId)).size===p.facts.length,'actual Hook facts missing');
  const facts=p.facts.map(f=>{const r=ledger.get(f.eventId);nvNeed(r&&f.sequence===String(r.event_sequence)&&f.observedAtMs===String(r.observed_at_ms)&&['code','level','message'].every(k=>f[k]===r[k]),'actual Hook facts differ');return r;});
  const first=codes=>facts.filter(r=>codes.includes(r.code)).map(r=>nvI64(r.event_sequence)).reduce((a,b)=>a===null||b<a?b:a,null);
  const handshake=first(['host_ready','host_response']),connected=first(['capture_connected','process_connected']),source=first(['stream_created','source_discovered','text_received']);
  nvNeed(handshake!==null&&connected!==null&&source!==null&&handshake<connected&&connected<source,'actual handshake/connection/source order missing');
  const projection=nvSource(candidate);nvNeed(projection.projection.sources.length&&candidate.text.includes('选择正文')&&!candidate.journal.some(e=>e.action==='port_call'&&e.method==='beginReading'),'actual candidates missing or reading premature');
  if(by.has('waiting')){const w=by.get('waiting'),prior=qualified.filter(([,b])=>b.observed_at_ms<=w.os.observed_at_ms);nvNeed(nvEqual(nvProfile(w),profile)&&w.context.session_id===sid&&prior.length&&prior.at(-1)[2].operationId===op&&prior.at(-1)[2].stage==='waiting_text'&&w.text.includes('等待')&&!w.text.includes('选择正文')&&!w.tables.capture_sources.some(s=>nvI64(s.discovered_at_ms??0)<=BigInt(w.os.observed_at_ms))&&w.os.observed_at_ms<candidate.os.observed_at_ms,'actual source-free waiting interval absent');return 'waiting-observed';}
  nvNeed(nvEqual(steps.map(s=>s.id),['candidate'])&&returned.observed_at_ms<=candidate.os.observed_at_ms&&projection.returned.observed_at_ms<=candidate.os.observed_at_ms,'actual fast source cutoff missing');return 'fast-candidates';
}
export function nativeObservationVerdict(caseId,steps) {
  try {
    nvInventory(caseId,steps);if(!nativeAmendments.has(caseId))return {case_id:caseId,status:'not_applicable',errors:[]};
    const by=Object.fromEntries(steps.map(s=>[s.id,s]));
    if(caseId==='BJ-13')nvSuccess(by.before,by.completed);
    if(caseId==='BJ-06')nvProgress(steps);
    if(caseId==='BJ-08'){
      nvNeed(['sessions','source_selection_sets','source_selection_members'].every(k=>nvEqual(by.before.tables[k],by.draft.tables[k]))&&!by.draft.calls.some(e=>e.action==='port_call'&&e.method==='beginReading')&&by.draft.text.includes('开始阅读'),'draft committed or begin unavailable');
      const proof=nvSuccess(by.before,by.committed);nvNeed(proof.members.length>=2&&by.committed.calls.filter(e=>e.action==='port_call'&&e.method==='beginReading').length===1,'current multisource Native commit missing');}
    if(caseId==='BJ-09'){
      const saved=step=>step.tables.source_selection_sets.filter(r=>r.persistence_kind==='saved_snapshot'),baseline=nvSelection(by.before);
      nvNeed(saved(by.before).length&&nvEqual(saved(by.before),saved(by.temporary))&&baseline.row.persistence_kind==='saved_snapshot','real nonempty saved baseline/preservation absent');saved(by.before).forEach(r=>nvSaved(by.before,r));
      nvNeed(nvSuccess(by.before,by.temporary).row.persistence_kind==='temporary','temporary outcome differs');const intent=nvReopen(by.temporary,by.save_intent);nvNeed(intent.rememberAllowed===true&&by.save_intent.text.includes('记住选择'),'owned remember intent unavailable');
      const proof=nvSuccess(by.save_intent,by.saved);nvNeed(proof.call.args.remember===true&&proof.row.persistence_kind==='saved_snapshot'&&proof.call.observed_at_ms>by.save_intent.os.observed_at_ms,'real later remember intent missing');nvSaved(by.saved,proof.row);
      const reopened=nvReopen(by.saved,by.saved_reopened);nvNeed(reopened.rememberAllowed===true&&nvEqual(reopened.current,proof.receipt.selection),'saved reopen current set differs');
      const unowned=nvSource(by.unowned);nvNeed(unowned.projection.rememberAllowed===false&&by.unowned.tables.sessions.length===1&&by.unowned.tables.sessions[0].game_id===null,'unowned borrowed game authority');
      for(const [a,b] of nvPairs(by.unowned.journal,'beginReading'))if(a.args?.sessionId===by.unowned.context.session_id&&a.args.remember===true)nvNeed(b.response?.errorCode==='remember_requires_game','unowned remember downgraded');}
    return {case_id:caseId,status:'passed',errors:[]};
  } catch(error){return {case_id:caseId,status:'failed',errors:[String(error.message??error)]};}
}

async function nvRead(root,reference,directory=null,limit=512*1024*1024) {
  nvNeed(reference&&nvEqual(Object.keys(reference).sort(),['path','sha256'])&&typeof reference.path==='string'&&/^[a-f0-9]{64}$/.test(reference.sha256),'Native exact original file reference required');
  const parts=reference.path.split('/');nvNeed(!path.isAbsolute(reference.path)&&!reference.path.includes('\\')&&parts.every(p=>p&&p!=='.'&&p!=='..'),'unsafe Native reference');
  let file=root;for(const p of parts){file=path.join(file,p);nvNeed(!(await fs.lstat(file)).isSymbolicLink(),'Native reference follows symlink');}
  nvNeed(!directory||file.startsWith(directory+path.sep),'Native evidence escaped invocation');
  const info=await fs.stat(file);nvNeed(info.isFile()&&info.size>=0&&info.size<=limit,'Native original file missing/unbounded');
  const bytes=await fs.readFile(file);nvNeed(createHash('sha256').update(bytes).digest('hex')===reference.sha256,'Native original bytes hash differs');return {file,bytes};
}
async function nvJson(root,reference,directory=null) {const data=await nvRead(root,reference,directory);return {file:data.file,value:strictProtocolJson(data.bytes.toString('utf8'))};}
async function nvCommand(root,reference,directory) {
  const {value:r}=await nvJson(root,reference,directory);nvNeed(Array.isArray(r.command)&&r.command.length&&r.command.every(x=>typeof x==='string')&&r.exit_code===0,'actual successful Native command receipt missing');
  const start=Date.parse(r.started_at),end=Date.parse(r.finished_at);nvNeed(Number.isFinite(start)&&Number.isFinite(end)&&start<=end&&/[zZ]|[+-]\d\d:\d\d$/.test(r.started_at)&&/[zZ]|[+-]\d\d:\d\d$/.test(r.finished_at),'actual Native command interval missing');
  await nvRead(root,r.stdout,directory);await nvRead(root,r.stderr,directory);return {...r,start_ms:start,end_ms:end};
}
function nvActionCommand(action,pid) {
  let body='set frontmost to true\nkeystroke "1" using command down';
  if(action==='open_current_sources')body='set hits to {}\nrepeat with element in (entire contents of window "Luna")\ntry\nif role of element is "AXButton" and name of element is "重新选择文本流…" then set end of hits to element\nend try\nend repeat\nif (count hits) is not 1 then error "source button identity ambiguous"\nclick item 1 of hits';
  else nvNeed(action==='normal_show_main','unregistered normal Native action');
  return ['/usr/bin/osascript','-e',`tell application "System Events"\ntell (first application process whose unix id is ${pid})\n${body}\nend tell\nend tell`];
}
function nvJournal(rows,m,expectedSource) {
  const first=rows[0];nvNeed(first?.kind==='binding_journal_begin'&&first.schema_version===1&&first.nonce===m.nonce&&first.pid===m.main_pid&&first.profile_id===m.profile_id&&first.source_tree_sha256===expectedSource,'actual Native application journal cohort differs');
  let sequence=0;const events=[],contexts=new Map(),fields=['review_id','session_id','operation_id','selection_set_revision','active_target_identity_sha256','candidate_target_identity_sha256'];
  const actions=new Set(['context_transition','intent','port_call','port_return','persist_commit','show_hud_completed','hide_main_completed','window_effect_failed','operation_cancelled']);
  for(const r of rows.slice(1)){
    nvNeed(r.nonce===first.nonce&&r.pid===first.pid&&r.profile_id===first.profile_id&&r.boot_sec===first.boot_sec&&r.boot_usec===first.boot_usec,'Native journal cohort changed');
    if(r.kind==='binding_journal_health')nvNeed(['accepted','written','last_sequence'].every(k=>Number.isSafeInteger(r[k])&&r[k]===sequence)&&['dropped','invalid','write_failures'].every(k=>r[k]===0),'Native journal loss/health differs');
    else {nvNeed(r.kind==='binding_journal_event'&&Number.isSafeInteger(r.sequence)&&r.sequence===sequence+1&&Number.isSafeInteger(r.observed_at_ms)&&m.started_at_ms<=r.observed_at_ms&&r.observed_at_ms<=m.finished_at_ms&&actions.has(r.action)&&typeof r.correlation_id==='string'&&r.correlation_id,'Native journal actual sequence/time/action differs');
      nvNeed(!['milestones','expected','passed','verdict'].some(k=>k in r),'Native producer verdict substituted');
      for(const k of fields.slice(0,4))nvNeed(r[k]===null||(typeof r[k]==='string'&&r[k]),'Native allocated context type differs');
      const c=Object.fromEntries(fields.map(k=>[k,r[k]??null]));if(r.action==='context_transition')contexts.set(r.correlation_id,c);else nvNeed(nvEqual(c,contexts.get(r.correlation_id)??Object.fromEntries(fields.map(k=>[k,null]))),'Native correlation context crossed');
      sequence++;events.push(r);}}
  nvNeed(sequence&&rows.at(-1).kind==='binding_journal_health','actual Native journal has no closed tail');return {header:first,events,health:rows.at(-1)};
}
function nvLsof(text) {
  const entries=[];let row={};for(const line of text.split('\n')){if(!line)continue;const key=line[0],value=line.slice(1);if(key==='p'||key==='f'){if(row.name)entries.push(row);row=key==='p'?{pid:Number(value)}:{pid:row.pid,fd:value};}else if(key==='n')row.name=value;else if(key==='i')row.inode=value;else if(key==='D'){nvNeed(/^0x[0-9a-fA-F]+$/.test(value),'Native lsof device is not exact hex');row.dev=String(BigInt(value));}}
  if(row.name)entries.push(row);return entries;
}
function nvHeldJournal(text,pid,journalPath,device=false) {
  nvNeed(nvEqual(text.split('\n').filter(line=>line.startsWith('p')),['p'+pid]),'Native journal FD process differs');
  const rows=nvLsof(text).filter(row=>row.name===journalPath);nvNeed(rows.length===1,'Native app-held journal FD absent or ambiguous');
  const row=rows[0];nvExactDecimal(row.inode);const value={pid:row.pid,fd:row.fd,path:row.name,inode:row.inode};
  if(device){nvExactDecimal(row.dev);value.dev=row.dev;}return value;
}
async function nvCollectJournalFence(root,raw,directory,journal,archive,tables,context,openedText,pid,execution) {
  const proof=nvObject(raw.journal_fence);nvNeed(nvEqual(Object.keys(proof).sort(),['query_correlation_id','fd_execution','archive_execution'].sort()),'Native actual post-query archive producer is missing');
  const fd=await nvCommand(root,proof.fd_execution,directory);
  nvNeed(nvEqual(fd.command,['/usr/sbin/lsof','-nP','-p',String(pid),'-F','pfDin']),'Native readonly fence FD observation command differs');
  const after=(await nvRead(root,fd.stdout,directory)).bytes.toString('utf8');
  const command=await nvCommand(root,proof.archive_execution,directory),node=await fs.realpath(process.execPath),expected=nativeJournalReadCommand(raw.actual_journal_path);
  expected[0]=node;nvNeed(nvEqual(command.command,expected),'Native fixed readonly journal reader command differs');
  const read=(await nvRead(root,command.stdout,directory,128*1024*1024)).bytes;
  const reader=strictProtocolJson(read.toString('utf8'));
  return {query_correlation_id:proof.query_correlation_id,cohort:Object.fromEntries(['nonce','pid','profile_id','boot_sec','boot_usec'].map(k=>[k,journal.header[k]])),archive,profile:nvProfile({tables}),tables,
    held_before:nvHeldJournal(openedText,pid,raw.actual_journal_path),held_after:nvHeldJournal(after,pid,raw.actual_journal_path,true),reader,
    fd_interval:[fd.start_ms,fd.end_ms],archive_interval:[command.start_ms,command.end_ms],archive_pid:command.pid,
    executable:{path:node,sha256:createHash('sha256').update(await fs.readFile(node)).digest('hex')},writer_interval:[execution.start_ms,execution.end_ms]};
}
function nvSqlite(database,table,inspections) {
  const names=new Set(['profile_meta','sessions','capture_operations','hook_events','capture_sources','source_selection_sets','source_selection_members','source_selections']);nvNeed(names.has(table),'unregistered Native SQLite table');
  const infoCommand=['/usr/bin/sqlite3','-readonly','-json',database,`PRAGMA table_info("${table}");`],info=spawnSync(infoCommand[0],infoCommand.slice(1),{encoding:'utf8',maxBuffer:1024*1024});
  inspections.push({command:infoCommand,exit_code:info.status});nvNeed(!info.error&&info.status===0,'Native actual SQLite columns unavailable');
  const columns=strictProtocolJson(info.stdout);nvNeed(columns.length&&columns.every(c=>typeof c.name==='string'&&/^[a-z][a-z0-9_]*$/.test(c.name)),'Native SQLite column identifier invalid');
  const exact=new Set(['selection_set_revision','selection_revision','runtime_generation','commit_revision','selected_at_ms','event_sequence','discovered_at_ms']);
  const projection=columns.map(c=>exact.has(c.name)?`CAST("${c.name}" AS TEXT) AS "${c.name}"`:`"${c.name}"`).join(',');
  const command=['/usr/bin/sqlite3','-readonly','-json',database,`SELECT ${projection} FROM "${table}";`],r=spawnSync(command[0],command.slice(1),{encoding:'utf8',maxBuffer:128*1024*1024});
  inspections.push({command,exit_code:r.status});nvNeed(!r.error&&r.status===0,'independent Node SQLite original read failed');
  return r.stdout.trim()?strictProtocolJson(r.stdout):[];
}
export function inspectNativeSqliteFacts(database,table) {const inspections=[];return {rows:nvSqlite(database,table,inspections),inspections};}
export function nativeObservationSampling(time,intervals,events,context,fence) {return nvSampling(time,intervals,events,context,fence);}
export async function inspectNativeRawCollection({root,reference,nonce,source_tree_sha256}) {
  root=await fs.realpath(root);const {file,value:m}=await nvJson(root,reference),directory=path.dirname(file),inspections=[];
  nvNeed(m.kind==='binding-journey-native-collection/v1'&&m.schema_version===1&&m.nonce===nonce&&m.source_tree_sha256===source_tree_sha256&&m.fixture_substitution===false,'Native raw collection source/nonce/origin differs');
  const ascii=value=>JSON.stringify(value).replace(/[\u007f-\uffff]/g,c=>'\\u'+c.charCodeAt(0).toString(16).padStart(4,'0'));
  nvNeed(m.source_map&&Object.values(m.source_map).every(v=>typeof v==='string'&&/^[a-f0-9]{64}$/.test(v)),'Native actual source map missing');
  const serialized='{'+Object.keys(m.source_map).sort().map(k=>ascii(k)+': '+ascii(m.source_map[k])).join(', ')+'}';
  nvNeed(createHash('sha256').update(serialized).digest('hex')===source_tree_sha256,'Native source map actual tree differs');
  const rows=[],recordBytes=[];for(const r of m.binding_journal??[]){const {bytes}=await nvRead(root,r,directory,64*1024*1024);for(const line of nvRawLines(bytes)){recordBytes.push(line);rows.push(strictProtocolJson(line.toString('utf8')));}}
  const journal=nvJournal(rows,m,source_tree_sha256);
  const journalBytes=Buffer.concat(recordBytes),archiveProof={sha256:createHash('sha256').update(journalBytes).digest('hex'),size:String(journalBytes.length),health:journal.health,records_base64:recordBytes.map(line=>line.toString('base64'))};
  const archive=await nvRead(root,m.artifact);const unzipped=spawnSync('/usr/bin/unzip',['-p',archive.file,'luna-build-identity.json'],{encoding:'utf8',maxBuffer:128*1024});
  nvNeed(!unzipped.error&&unzipped.status===0,'Native package identity original unavailable');const packageIdentity=strictProtocolJson(unzipped.stdout);
  nvNeed(packageIdentity.kind==='luna_packaged_build_identity'&&packageIdentity.source_tree_sha256===source_tree_sha256&&packageIdentity.bundle_id==='com.lunatranslator.reader','Native package identity differs');
  const results=[],found=new Set();
  for(const row of m.cases){const caseId=row.case_id;nvNeed(!found.has(caseId),'duplicate Native case');found.add(caseId);if(!nativeAmendments.has(caseId))continue;
    nvInventory(caseId,row.steps);nvNeed(!['branch','milestones','allow_main_absent','role'].some(k=>k in row),'Native case authorizes its own role');
    const execution=await nvCommand(root,row.execution,directory),binary=await nvRead(root,row.binary,directory,256*1024*1024);
    nvNeed(nvEqual(execution.command,[binary.file,'--ignored','--exact',caseId.toLowerCase().replace('-','_')+'_native','--nocapture'])&&execution.environment?.LUNA_BINDING_NONCE===nonce&&execution.environment?.LUNA_BINDING_SOURCE_TREE===source_tree_sha256,'actual Native fixed writer invocation differs');
    const actualEvents=journal.events.filter(e=>execution.start_ms<=e.observed_at_ms&&e.observed_at_ms<=execution.end_ms),bounds=row.journal_range;
    nvNeed(actualEvents.length&&nvEqual(bounds,{start_sequence:actualEvents[0].sequence,end_sequence:actualEvents.at(-1).sequence}),'actual Native writer interval not closed');
    const steps=[];let knownMain=null,knownHud=null,cohort=null;
    for(const raw of row.steps){const {file:osFile,value:os}=await nvJson(root,raw.os,directory),time=os.observed_at_ms;
      nvNeed(os.kind==='independent_macos_runtime_observation'&&os.schema_version===1&&os.screen_capture_preflight===true&&os.boot_status===0&&Number.isSafeInteger(time)&&execution.start_ms<=time&&time<=execution.end_ms&&os.boot_sec===journal.header.boot_sec&&os.boot_usec===journal.header.boot_usec,'Native original OS interval/boot differs');
      const processes=os.processes.filter(p=>p.pid===m.main_pid);nvNeed(processes.length===1,'Native exact app PID missing');const process=processes[0];
      nvNeed(process.guest_code?.validity===0&&process.guest_code?.identifier==='com.lunatranslator.reader'&&process.guest_code?.cdhash===packageIdentity.signature_metadata?.cdhash,'Native OS package signature differs');
      const exe=await fs.readFile(process.executable);nvNeed(createHash('sha256').update(exe).digest('hex')===packageIdentity.executable_sha256,'Native actual installed executable differs');
      const bundle=path.dirname(path.dirname(path.dirname(process.executable))),sealedPath=path.join(bundle,'Contents/Resources/luna-source-identity.json');
      nvNeed(!(await fs.lstat(sealedPath)).isSymbolicLink(),'Native installed source identity follows symlink');
      const sealed=strictProtocolJson(await fs.readFile(sealedPath,'utf8'));nvNeed(sealed.kind==='luna_sealed_source_identity'&&sealed.schema_version===1&&sealed.source_tree_sha256===source_tree_sha256,'Native installed sealed source differs');
      const signatureCheck=spawnSync('/usr/bin/codesign',['--verify','--deep','--strict','--all-architectures',bundle],{encoding:'utf8',maxBuffer:1024*1024});inspections.push({command:['/usr/bin/codesign','--verify','--deep','--strict','--all-architectures',bundle],exit_code:signatureCheck.status});nvNeed(!signatureCheck.error&&signatureCheck.status===0,'Native installed resource signature invalid');
      const identity=[process.pid,process.start_sec,process.start_usec,process.executable,os.boot_sec,os.boot_usec];nvNeed(!cohort||nvEqual(cohort,identity),'Native OS process/start/boot changed');cohort=identity;
      const osCommand=await nvCommand(root,raw.os_execution,directory),observer=await nvRead(root,raw.observer_binary,directory,64*1024*1024),compile=await nvCommand(root,raw.observer_compilation,directory);
      nvNeed(nvEqual(compile.command,['/usr/bin/xcrun','swiftc',path.join(root,'scripts/quality/runtime_observer.swift'),'-o',observer.file])&&nvEqual(raw.observer_source,{path:'scripts/quality/runtime_observer.swift',sha256:m.source_map['scripts/quality/runtime_observer.swift']})&&osCommand.command.length===7&&nvEqual(osCommand.command.slice(0,3),[observer.file,'--output',osFile])&&osCommand.command[3]==='--pids'&&osCommand.command[4].split(',').includes(String(m.main_pid))&&osCommand.command[5]==='--windows','Native exact original observer chain differs');
      const requested=osCommand.command[6].split(',').map(Number),role=nativeObservationRole(caseId,raw.id),windows=os.windows;
      nvNeed(Number.isSafeInteger(raw.main_window_id)&&requested.includes(raw.main_window_id),'Native Main not requested');
      let main=windows.filter(w=>w.window_id===raw.main_window_id),hud=windows.filter(w=>w.window_id===raw.hud_window_id);
      if(role==='hud'){nvNeed(knownMain===raw.main_window_id&&knownHud===raw.hud_window_id&&main.length<=1&&(!main.length||(main[0].pid===m.main_pid&&main[0].on_screen===false))&&hud.length===1&&hud[0].pid===m.main_pid&&hud[0].on_screen===true&&requested.includes(raw.hud_window_id),'Native known Main hidden/HUD visible not established');main=main.length?main:[{pid:m.main_pid,window_id:raw.main_window_id,on_screen:false}];}
      else {nvNeed(main.length===1&&main[0].pid===m.main_pid&&main[0].on_screen===true&&(!knownMain||knownMain===raw.main_window_id),'Native Main absent or changed');knownMain=raw.main_window_id;if(hud.length===1&&hud[0].pid===m.main_pid){nvNeed(!knownHud||knownHud===raw.hud_window_id,'Native HUD changed');knownHud=raw.hud_window_id;}}
      const capture=(role==='hud'?hud[0]:main[0]).capture;nvNeed(capture?.png&&Array.isArray(capture.ocr)&&capture.ocr.length,'Native role original PNG/OCR missing');
      for(const w of windows){const cap=w.capture??{};for(const image of [...(cap.png?[cap]:[]),...(cap.preview_crops??[])]){nvNeed(path.basename(image.png)===image.png,'Native image escaped capture');const imageRef=raw.images?.[image.png],actualPath=path.relative(root,path.join(path.dirname(osFile),image.png)).split(path.sep).join('/');nvNeed(imageRef?.path===actualPath,'Native image original inventory differs');const {bytes}=await nvRead(root,imageRef,directory,64*1024*1024);nvNeed(bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))&&bytes.readUInt32BE(16)===image.width&&bytes.readUInt32BE(20)===image.height,'Native original PNG dimensions differ');}}
      const watermark=Math.max(0,...journal.events.filter(e=>e.observed_at_ms<=time).map(e=>e.sequence));nvNeed(Number.isSafeInteger(raw.journal_end_sequence)&&raw.journal_end_sequence===watermark&&bounds.start_sequence<=watermark&&watermark<=bounds.end_sequence&&!journal.events.some(e=>e.sequence<=watermark&&e.observed_at_ms>time),'Native OS complete watermark differs');
      const events=journal.events.filter(e=>bounds.start_sequence<=e.sequence&&e.sequence<=watermark),transitions=events.filter(e=>e.action==='context_transition');nvNeed(transitions.length,'Native actual context missing');
      const fields=['review_id','session_id','operation_id','selection_set_revision','active_target_identity_sha256','candidate_target_identity_sha256'],context=Object.fromEntries(fields.map(k=>[k,transitions.at(-1)[k]??null]));nvNeed(nvEqual(raw.context,context),'Native context differs from original journal');
      const calls=events.filter(e=>['port_call','port_return'].includes(e.action)),callBytes=await nvJson(root,raw.calls,directory);nvNeed(nvEqual(callBytes.value,calls),'Native calls do not project original app journal');
      const database=await nvRead(root,raw.database,directory),backup=await nvCommand(root,raw.database_execution,directory),opened=await nvCommand(root,raw.lsof_execution,directory);
      nvNeed(path.isAbsolute(raw.actual_database)&&nvEqual(backup.command,['/usr/bin/sqlite3',raw.actual_database,'.backup '+database.file])&&nvEqual(opened.command,['/usr/sbin/lsof','-nP','-p',String(m.main_pid),'-F','pfin']),'Native actual app-open DB backup chain differs');
      const openedData=(await nvRead(root,opened.stdout,directory)).bytes.toString('utf8'),entries=nvLsof(openedData);nvNeed(nvEqual(openedData.split('\n').filter(l=>l.startsWith('p')),['p'+m.main_pid])&&entries.some(e=>e.name===raw.actual_database)&&entries.some(e=>e.name===raw.actual_journal_path&&e.inode===raw.actual_journal_inode),'Native app did not hold exact original DB/journal FD');
      const inodes=[...new Set(entries.filter(e=>e.name===raw.actual_database).map(e=>e.inode))];nvNeed(inodes.length===1&&inodes[0],'Native actual database FD/inode ambiguous');
      const intervals={os:[osCommand.start_ms,osCommand.end_ms],database:[backup.start_ms,backup.end_ms],lsof:[opened.start_ms,opened.end_ms]};
      nvNeed(Object.values(intervals).every(p=>execution.start_ms<=p[0]&&p[1]<=execution.end_ms),'Native capture commands escaped fixed writer interval');
      const tables={};for(const table of ['profile_meta','sessions','capture_operations','hook_events','capture_sources','source_selection_sets','source_selection_members','source_selections']){const all=nvSqlite(database.file,table,inspections);tables[table]=table==='profile_meta'||table==='sessions'&&context.session_id===null?all:all.filter(r=>context.session_id!==null&&r.session_id===context.session_id);}
      const fence=await nvCollectJournalFence(root,raw,directory,journal,archiveProof,tables,context,openedData,m.main_pid,execution);
      const sampling=nvSampling(time,intervals,journal.events,context,fence);
      const actions=[];for(const action of raw.native_actions??[]){nvNeed(nvEqual(Object.keys(action).sort(),['execution','id']),'Native free reopen action metadata');const r=await nvCommand(root,action.execution,directory);nvNeed(nvEqual(r.command,nvActionCommand(action.id,m.main_pid))&&m.started_at_ms<=r.start_ms&&r.end_ms<=time,'Native actual normal action differs');actions.push({id:action.id,started_at_ms:r.start_ms,finished_at_ms:r.end_ms});}
      nvNeed(!actions.length||(caseId==='BJ-09'&&['save_intent','saved_reopened'].includes(raw.id)),'Native normal reopen outside fixed edge');
      const text=[...capture.ocr.map(r=>r.text),...(process.ax??[]).flatMap(r=>['title','value','description'].map(k=>String(r[k]??'')))].join('');
      const step={id:raw.id,process,os,main:main[0],hud:hud[0]??null,text,tables,calls,journal:events,context,raw,native_actions:actions,sampling,database_owner:{pid:m.main_pid,path:raw.actual_database,inode:inodes[0],journal_path:raw.actual_journal_path,journal_inode:raw.actual_journal_inode}};nvNeed(nvProfile(step).profileId===journal.header.profile_id,'Native SQLite profile differs from app journal');steps.push(step);}
    results.push(nativeObservationVerdict(caseId,steps));}
  return {kind:'independent-node-native-observation-inspection/v1',results,inspections,native_execution_performed:false};
}
// End independent Native observation consumer.
