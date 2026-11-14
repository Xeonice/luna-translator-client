/** Manual UI regressions on real components, with isolated in-memory input. */
import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { launchLockedBrowser, verifyBrowserIdentity } from '../visual/locked-browser.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const output = path.join(root, '.build/current-client-execution-20261105/manual-translation-alignment/implementation/run-' + Date.now());
const workspace = path.join(output, 'workspace');
const desktop = path.join(workspace, 'apps/desktop');
const results = [];
const tracked = ['apps/desktop/src/ui/primitives/Controls.tsx', 'apps/desktop/src/features/reading/ReadingPanel.tsx', 'apps/desktop/src/features/reading/reading-typography.ts', 'apps/desktop/src/features/reading/index.ts', 'apps/desktop/src/features/manual-input/ManualInputScreen.tsx', 'apps/desktop/src/features/manual-input/styles.module.css', 'apps/desktop/src/features/model-config/ServiceSelector.tsx', 'apps/desktop/src/features/model-config/service-selector.module.css', 'apps/desktop/tests/live-preview/verify-manual-ui-alignment.mjs'];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let browser, identity, server, origin, serverLog = '', fixedServerPid, sourceBefore;
const check = (name, run) => test(name, { timeout: 30000 }, async () => {
  try { const value = await run(); results.push({ name, status: 'passed', value }); }
  catch (error) { results.push({ name, status: 'failed', error: error.stack }); throw error; }
});
async function fixedHealth() {
  try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(500) })).json(); }
  catch { return null; }
}
async function record(name, value) { await fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n'); }
async function open(role = 'main', view = 'manual', width = 1040, height = 720) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  page.setDefaultTimeout(6000);
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${origin}/client.html?role=${role}&view=${view}&state=normal&theme=dark`);
  await page.waitForFunction(() => Boolean(window.liveUiPreview?.port));
  if (role === 'main') await page.locator('nav [aria-current="page"]').filter({ hasText: view === 'manual' ? '手动翻译' : '当前会话' }).waitFor();
  else await page.locator('#root > *').waitFor();
  return { page, errors };
}
async function assertNoErrors(page, errors, image) {
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: path.join(output, image + '.png') });
  assert.deepEqual(errors, []); await page.close();
}

before(async () => {
  await fs.mkdir(output, { recursive: true });
  sourceBefore = Object.fromEntries(await Promise.all(tracked.map(async p => [p, hash(await fs.readFile(path.join(root, p)))])));
  await record('source-before', sourceBefore);
  for (const relative of ['apps/desktop/src', 'apps/desktop/tests/support', 'apps/desktop/tests/live-preview', 'apps/desktop/tsconfig.json', 'apps/desktop/package.json', 'docs/design/first-product-v1/assets']) {
    const destination = path.join(workspace, relative); await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.cp(path.join(root, relative), destination, { recursive: true });
  }
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir');
  await fs.symlink(path.join(root, 'node_modules'), path.join(workspace, 'node_modules'), 'dir');
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = `http://127.0.0.1:${port}`;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', path.join(desktop, 'tests/live-preview/vite.config.ts'), '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', chunk => { serverLog += chunk; }); server.stderr.on('data', chunk => { serverLog += chunk; });
  for (let index = 0; index < 150; index++) {
    try { if ((await fetch(origin)).ok) break; } catch {}
    if (index === 149 || server.exitCode !== null) throw new Error('Isolated preview did not start: ' + serverLog);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  ({ browser, identity } = await launchLockedBrowser());
  fixedServerPid = (await fixedHealth())?.pid ?? null;
});

after(async () => {
  if (browser) { await verifyBrowserIdentity(identity); await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('exit', resolve)); }
  const fixedAfter = (await fixedHealth())?.pid ?? null;
  const sourceAfter = Object.fromEntries(await Promise.all(tracked.map(async p => [p, hash(await fs.readFile(path.join(root, p)))])));
  await record('results', { kind: 'real-production-manual-ui-only', browser: identity, sourceBefore, sourceAfter, fixedServerPid, fixedAfter, cases: results, native: 'not_run', product_completion: 'incomplete' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  assert.equal(fixedAfter, fixedServerPid, 'The user preview process must not change');
  assert.deepEqual(sourceAfter, sourceBefore, 'Production source must remain stable during the isolated run');
  console.log('Manual UI evidence: ' + output);
});

// Explicit controlled transport responses preserve real WindowRoot, store,
// commands and reading projections; they do not run a translator or a timer.
async function seed(page, options = {}) {
  return page.evaluate(options => {
    const p = window.liveUiPreview.port;
    const original = p.state.current ?? p.state.history.units[0];
    const unit = { ...structuredClone(original), unitId: options.unitId ?? 'manual-owned-result', sessionId: 'manual-owned-session', gameId: null };
    unit.sourceSummary = options.unknown ? null : { sessionId: options.mismatched ? 'different-session' : unit.sessionId, gameId: null, sourceKind: options.sourceKind ?? 'manual', state: 'stopped', startedAtMs: unit.createdAtMs, endedAtMs: unit.createdAtMs };
    const task = { taskId: unit.unitId + ':task', unitId: unit.unitId, textRevision: unit.textRevision, purpose: 'translation', status: options.phase ?? 'completed', errorCode: null, partialText: options.phase === 'running' ? '受控翻译片段' : null, providerLabel: '手动受控服务', attemptId: unit.unitId + ':attempt' };
    unit.translations = options.phase ? [] : [{ translationId: unit.unitId + ':translation', taskId: task.taskId, text: '手动受控译文', providerLabel: task.providerLabel, targetLanguage: options.targetLanguage ?? 'en', correction: null }];
    const commitRevision = p.state.commitRevision + 1;
    const projection = { profileId: p.state.profileId, serverInstanceId: p.state.serverInstanceId, profileGeneration: p.state.profileGeneration, commitRevision, unit: structuredClone(unit), translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines: [{ scopeKey: 'global', bindingId: 'manual-binding', engineId: 'manual-engine', providerLabel: task.providerLabel, status: { kind: 'current', task } }] } };
    const key = JSON.stringify([unit.unitId, unit.textRevision]);
    p.readingProjectionFixtures[key] = projection; p.readingUnitFixtures[key] = structuredClone(unit);
    p.publish({ current: unit, history: { units: [unit, ...p.state.history.units.filter(value => value.unitId !== unit.unitId)], nextCursor: null }, commitRevision, tasks: [task], ...(options.preferences ? { preferences: { ...p.state.preferences, ...options.preferences } } : {}) });
    return structuredClone(unit);
  }, options);
}
async function installManualTransport(page, { fail = false, hold = false, receiptHook = false } = {}) {
  await page.evaluate(({ fail, hold, receiptHook }) => {
    const p = window.liveUiPreview.port, original = p.dispatch.bind(p);
    p.dispatch = async envelope => {
      if (envelope.command.kind !== 'manual_input') return original(envelope);
      p.commands.push(structuredClone(envelope));
      if (hold) await new Promise(resolve => { window.manualRelease = resolve; });
      if (fail) throw new Error('controlled manual save failure');
      const base = structuredClone(p.state.current);
      const unit = { ...base, unitId: 'manual-saved-receipt', sessionId: 'manual-saved-session', gameId: null, sourceText: envelope.command.payload.text, sourceLanguage: envelope.command.payload.sourceLanguage, tokens: [], translations: [], sourceSummary: { sessionId: 'manual-saved-session', gameId: null, sourceKind: receiptHook ? 'hook' : 'manual', state: 'stopped', startedAtMs: base.createdAtMs, endedAtMs: base.createdAtMs } };
      p.readingUnitFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = structuredClone(unit);
      p.publish({ history: { units: [unit, ...p.state.history.units], nextCursor: null }, commitRevision: p.state.commitRevision + 1 });
      return { requestId: envelope.operationId, revision: p.state.commitRevision, replayed: false, resultId: unit.unitId };
    };
  }, { fail, hold, receiptHook });
}
const result = page => page.getByRole('region', { name: '手动翻译结果', exact: true });

check('unassociated Hook and unknown or mismatched sources never become manual results', async () => {
  const { page, errors } = await open();
  // The normal production DesignPort already supplies gameId:null + hook.
  assert.equal(await page.locator('[data-testid="source-text"]:visible').count(), 0, 'Hook current with no game must stay out of manual');
  for (const options of [{ sourceKind: 'hook' }, { unknown: true }, { mismatched: true }]) {
    await seed(page, options);
    await page.evaluate(() => new Promise(requestAnimationFrame));
    assert.equal(await result(page).locator('[data-testid="source-text"]').count(), 0);
    await result(page).getByText('译文将在这里显示。', { exact: true }).waitFor();
  }
  await assertNoErrors(page, errors, 'manual-source-guard');
});

check('permanent empty result, header settings and compact selection follow manual layout at both viewports', async () => {
  const facts = [];
  for (const [width, height, x, y, editorWidth] of [[1040, 720, 217, 199.03125, 796], [760, 560, 84, 193.03125, 649]]) {
    const { page, errors } = await open('main', 'manual', width, height);
    await seed(page, { sourceKind: 'hook' });
    await result(page).getByText('译文将在这里显示。', { exact: true }).waitFor();
    const area = page.getByRole('textbox', { name: '原文', exact: true });
    assert.equal(await area.evaluate(node => node.rows), 6);
    const geometry = await area.evaluate(node => { const b = node.getBoundingClientRect(), css = getComputedStyle(node); return { x: b.x, y: b.y, width: b.width, height: b.height, fontSize: css.fontSize, lineHeight: css.lineHeight, padding: css.padding, radius: css.borderRadius, background: css.backgroundColor, filter: css.backdropFilter }; });
    assert.equal(geometry.x, x); assert.equal(geometry.y, y); assert.equal(geometry.width, editorWidth); assert.ok(Math.abs(geometry.height - 184.78125) < .1); assert.equal(geometry.fontSize, '16px'); assert.equal(geometry.lineHeight, '28.8px'); assert.equal(geometry.padding, '5px 8px'); assert.equal(geometry.radius, '13px'); assert.equal(geometry.background, 'rgba(58, 58, 68, 0.863)'); assert.equal(geometry.filter, 'blur(20px) saturate(0.9)');
    const region = await result(page).boundingBox(); assert.equal(region.y, y + 253.28125); assert.equal(region.height, 100);
    const actions = page.getByRole('group', { name: '手动输入操作', exact: true }); assert.deepEqual(await actions.getByRole('button').allTextContents(), ['翻译', '清空']);
    await page.locator('header').filter({ has: page.getByRole('heading', { name: '手动翻译', exact: true }) }).getByRole('button', { name: '翻译设置…', exact: true }).waitFor();
    const selector = page.locator('[data-service-appearance="manual"]'); assert.equal(await selector.count(), 1); const selectorBox = await selector.boundingBox(); assert.ok(selectorBox.height <= 30);
    assert.equal(await page.getByRole('combobox', { name: '原文语言', exact: true }).getAttribute('data-select-value'), 'ja');
    assert.match(await page.locator('[data-manual-languages]').innerText(), /目标语言随翻译服务设置/);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    const overflow = await page.locator('[data-manual-body]').evaluate(node => node.scrollWidth > node.clientWidth); assert.equal(overflow, false);
    facts.push({ width, geometry, result: region, selector: selectorBox });
    await assertNoErrors(page, errors, `manual-layout-${width}`);
  }
  return facts;
});

check('manual source fallback preserves actual languages, lexical words, copy and frozen learning', async () => {
  const { page, errors } = await open(); const unit = await seed(page, { targetLanguage: 'fr' });
  await result(page).getByText('手动受控译文', { exact: true }).waitFor();
  assert.match(await page.locator('[data-manual-languages]').innerText(), /fr/);
  const token = result(page).locator('[data-testid="source-text"] button').first(); await token.click();
  await page.waitForFunction(() => window.liveUiPreview.port.wordCardRequests.length === 1);
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.wordCardRequests[0].unitId), unit.unitId);
  await page.keyboard.press('Escape');
  await result(page).getByRole('button', { name: '复制原文', exact: true }).click();
  await result(page).getByRole('button', { name: '精读本句', exact: true }).click();
  await page.waitForFunction(() => Boolean(window.liveUiPreview.port.state.learningObject));
  const ownership = await page.evaluate(() => ({ copy: window.liveUiPreview.port.copiedReadingUnits[0], study: window.liveUiPreview.port.state.learningObject.unit }));
  assert.equal(ownership.copy.unitId, unit.unitId); assert.equal(ownership.copy.textRevision, unit.textRevision); assert.equal(ownership.copy.text, unit.sourceText); assert.equal(ownership.study.unitId, unit.unitId); assert.equal(ownership.study.sourceText, unit.sourceText);
  await assertNoErrors(page, errors, 'manual-real-reading-actions'); return ownership.copy;
});

check('source language uses real manual command and submitted receipt wins over current fallback', async () => {
  const { page, errors } = await open(); await seed(page); await installManualTransport(page);
  const area = page.getByRole('textbox', { name: '原文', exact: true }); await area.fill('  English source remains exact.\nSecond line.  ');
  const language = page.getByRole('combobox', { name: '原文语言', exact: true }); await language.click(); await page.getByRole('option', { name: '英语', exact: true }).click();
  await page.getByRole('button', { name: '翻译并阅读', exact: true }).click();
  await result(page).getByTestId('source-text').filter({ hasText: 'English source remains exact.' }).waitFor();
  const command = await page.evaluate(() => window.liveUiPreview.port.commands.find(item => item.command.kind === 'manual_input').command);
  assert.deepEqual(command.payload, { text: '  English source remains exact.\nSecond line.  ', sourceLanguage: 'en' });
  assert.equal(await area.inputValue(), command.payload.text); assert.equal(await result(page).getByTestId('source-text').count(), 1);
  await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.getByRole('button', { name: '手动翻译', exact: true }).click(); assert.equal(await area.inputValue(), command.payload.text);
  await language.click(); await page.getByRole('option', { name: '中文', exact: true }).click(); assert.equal(await language.getAttribute('data-select-value'), 'zh-CN');
  await assertNoErrors(page, errors, 'manual-submit-language-and-receipt'); return command;
});

check('manual save wait and failure preserve editable input without pretending model cancellation', async () => {
  const { page, errors } = await open(); await installManualTransport(page, { fail: true, hold: true });
  const area = page.getByRole('textbox', { name: '原文', exact: true }); await area.fill('保存前の原文。');
  const translate = page.getByRole('button', { name: '翻译并阅读', exact: true }); await translate.click();
  await page.waitForFunction(() => Boolean(window.manualRelease)); await page.getByRole('status').filter({ hasText: '正在保存原文' }).waitFor();
  assert.equal(await translate.isDisabled(), true); assert.equal(await page.getByRole('button', { name: '清空输入', exact: true }).isDisabled(), true); assert.equal(await area.isEnabled(), true);
  await area.fill('保存中も編集した原文。'); assert.equal(await page.getByRole('button', { name: '取消', exact: true }).count(), 0);
  await page.evaluate(() => window.manualRelease()); await page.getByRole('alert').filter({ hasText: 'controlled manual save failure' }).waitFor(); assert.equal(await area.inputValue(), '保存中も編集した原文。');
  await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.getByRole('button', { name: '手动翻译', exact: true }).click(); assert.equal(await area.inputValue(), '保存中も編集した原文。');
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.commands.some(item => item.command.kind === 'cancel_task')), false);
  await assertNoErrors(page, errors, 'manual-save-failure-keeps-draft');
});

check('a submitted receipt cannot promote a Hook row into the manual result', async () => {
  const { page, errors } = await open(); await installManualTransport(page, { receiptHook: true });
  await page.getByRole('textbox', { name: '原文', exact: true }).fill('receipt hook must be rejected'); await page.getByRole('button', { name: '翻译并阅读', exact: true }).click();
  await page.waitForFunction(() => window.liveUiPreview.port.state.history.units.some(unit => unit.unitId === 'manual-saved-receipt') && document.querySelector('[aria-label="翻译并阅读"]').getAttribute('aria-busy') !== 'true');
  await result(page).getByText('译文将在这里显示。', { exact: true }).waitFor(); assert.equal(await result(page).getByTestId('source-text').count(), 0);
  await assertNoErrors(page, errors, 'manual-receipt-source-guard');
});

check('a manual receipt from an earlier profile generation cannot select a reused history id', async () => {
  const { page, errors } = await open(); await installManualTransport(page);
  await page.getByRole('textbox', { name: '原文', exact: true }).fill('profile owned source');
  await page.getByRole('button', { name: '翻译并阅读', exact: true }).click();
  await result(page).getByTestId('source-text').filter({ hasText: 'profile owned source' }).waitFor();
  await page.evaluate(() => { const p = window.liveUiPreview.port, reused = structuredClone(p.state.history.units[0]); reused.sourceText = 'different profile reused id'; reused.tokens = []; const current = structuredClone(p.state.current); current.sourceSummary.sourceKind = 'hook'; p.publish({ profileGeneration: p.state.profileGeneration + 1, current, history: { units: [reused], nextCursor: null } }); });
  await result(page).getByText('译文将在这里显示。', { exact: true }).waitFor();
  assert.equal(await result(page).getByTestId('source-text').count(), 0);
  await assertNoErrors(page, errors, 'manual-receipt-profile-ownership');
});

check('clear retains saved reading and queued/running cancellation uses the actual task id', async () => {
  const { page, errors } = await open();
  for (const phase of ['queued', 'running']) {
    const unit = await seed(page, { phase, unitId: 'manual-' + phase });
    const before = await page.evaluate(() => window.liveUiPreview.port.state.history.units.length);
    await page.getByRole('textbox', { name: '原文', exact: true }).fill('清空される下書き'); await page.getByRole('button', { name: '清空输入', exact: true }).click();
    assert.equal(await page.getByRole('textbox', { name: '原文', exact: true }).inputValue(), ''); assert.equal(await page.evaluate(() => window.liveUiPreview.port.state.history.units.length), before);
    assert.equal(await result(page).getByTestId('source-text').count(), 1);
    assert.equal(await page.evaluate(() => window.liveUiPreview.port.commands.filter(item => item.command.kind === 'cancel_task').length), phase === 'queued' ? 0 : 1);
    await page.evaluate(() => { const p = window.liveUiPreview.port, unit = p.state.current, key = JSON.stringify([unit.unitId, unit.textRevision]), projection = structuredClone(p.readingProjectionFixtures[key]); projection.commitRevision = p.state.commitRevision + 1; const task = projection.translationState.engines[0].status.task; task.status = 'cancelled'; task.partialText = null; const snapshot = structuredClone(p.state); snapshot.commitRevision = projection.commitRevision; snapshot.tasks = [task]; p.readingProjectionCancellations.push({ taskId: task.taskId, snapshot, projection, receipt: { requestId: 'controlled-cancel', revision: snapshot.commitRevision, replayed: false, resultId: null } }); });
    await result(page).getByRole('button', { name: '取消', exact: true }).click(); await result(page).getByText(/已取消/).waitFor();
    const payload = await page.evaluate(() => window.liveUiPreview.port.commands.filter(item => item.command.kind === 'cancel_task').at(-1).command.payload); assert.deepEqual(payload, { taskId: unit.unitId + ':task' });
  }
  await assertNoErrors(page, errors, 'manual-clear-and-real-cancel');
});

check('manual selector reuses saved-service mutations and other appearances keep their recipe', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => { const p = window.liveUiPreview.port, source = { kind: 'http', source: { revisionId: 'manual-service-revision', modelId: 'manual-model' } }; window.manualSelections = []; p.serviceFixtures.saved = async query => ({ profileGeneration: p.state.profileGeneration, appliedHeadRevision: p.state.configuration.appliedHeadRevision, selection: { requestedScope: query.scope, effectiveScope: query.scope, selectionRevision: window.manualSelections.length, effectiveSelectionRevision: window.manualSelections.length, main: window.manualSelections.length ? source : null, comparisons: [], purposeEnabled: true }, rows: [{ logicalResourceId: 'manual-resource', source, purpose: 'translation', displayName: '手动已保存服务', backend: 'http_translation', verification: { kind: 'untested' }, eligibility: { kind: 'selectable' } }] }); p.serviceFixtures.select = async input => { window.manualSelections.push(input); return { operationId: input.operationId, revisionId: 'manual-service-revision', appliedHeadRevision: p.state.configuration.appliedHeadRevision, selectionRevision: 1, affectedTaskIds: [], semanticChange: true }; }; });
  const selector = page.locator('[data-service-appearance="manual"]'); await selector.getByRole('button').click();
  const dialog = page.getByRole('dialog', { name: '选择翻译服务', exact: true }); await dialog.getByRole('button', { name: '设为主服务', exact: true }).click(); await dialog.waitFor({ state: 'hidden' }); await selector.getByText('手动已保存服务', { exact: true }).waitFor();
  const selection = await page.evaluate(() => window.manualSelections[0]); assert.equal(selection.purpose, 'translation'); assert.deepEqual(selection.scope, { kind: 'global' }); assert.equal(selection.main.source.revisionId, 'manual-service-revision');
  const commandsBeforeSettings = await page.evaluate(() => window.liveUiPreview.port.commands.length);
  await page.getByRole('button', { name: '翻译设置…', exact: true }).click();
  await page.waitForFunction(() => window.liveUiPreview.port.settingsPageRequests.length === 1);
  assert.deepEqual(await page.evaluate(() => window.liveUiPreview.port.settingsPageRequests), [{ revision: 1, intent: 'translation' }]);
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.commands.length), commandsBeforeSettings);
  await assertNoErrors(page, errors, 'manual-saved-service-shared-path');
  for (const [role, view, appearance] of [['main', 'session', 'session'], ['hud', 'hud', 'hud'], ['settings', 'settings-translation', 'standard']]) {
    const context = await open(role, view, role === 'hud' ? 560 : 1040, role === 'hud' ? 290 : 720);
    if (appearance === 'standard') await context.page.locator('[data-configuration-section="translation"] > summary').click();
    const control = context.page.locator(`[data-service-appearance="${appearance}"]`).first(); await control.waitFor(); assert.equal(await context.page.locator('[data-service-appearance="manual"]').count(), role === 'main' ? 1 : 0);
    if (appearance === 'hud') assert.ok((await control.boundingBox()).height <= 30);
    await assertNoErrors(context.page, context.errors, 'manual-regression-' + appearance);
  }
  return selection;
});

check('shared multiline rows default is preserved for the real history correction editor', async () => {
  const { page, errors } = await open(); await seed(page);
  assert.equal(await page.getByRole('textbox', { name: '原文', exact: true }).evaluate(node => node.rows), 6);
  await page.getByRole('button', { name: '历史记录', exact: true }).click();
  await page.getByRole('button', { name: '修正 手动受控服务', exact: true }).click();
  const correction = page.getByRole('textbox', { name: '人工修正', exact: true }); await correction.waitFor();
  assert.equal(await correction.evaluate(node => node.rows), 2); assert.equal(await correction.getAttribute('rows'), null);
  assert.equal(await correction.inputValue(), '手动受控译文');
  await correction.fill('只编辑草稿，不保存'); await page.getByRole('button', { name: '取消编辑', exact: true }).click();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.commands.some(item => item.command.kind === 'save_correction')), false);
  await assertNoErrors(page, errors, 'manual-text-field-rows-default');
});

check('manual default and custom translation typography stays equal from streaming to completion without changing saved preferences', async () => {
  const { page, errors } = await open(); const facts = [];
  for (const [sourceFontPt, translationFontPt] of [[18, 24], [29, 35]]) {
    const preferences = { sourceFontPt, translationFontPt };
    await seed(page, { phase: 'running', preferences });
    const partial = result(page).getByText('受控翻译片段', { exact: true }); await partial.waitFor();
    const running = await partial.evaluate(node => parseFloat(getComputedStyle(node).fontSize));
    await seed(page, { preferences });
    const complete = result(page).getByText('手动受控译文', { exact: true }); await complete.waitFor();
    const completed = await complete.evaluate(node => parseFloat(getComputedStyle(node).fontSize));
    assert.ok(Math.abs(running - translationFontPt * 19 / 24) < .001, `stream ${running} must use manual role`); assert.ok(Math.abs(completed - translationFontPt * 19 / 24) < .001, `complete ${completed} must use manual role`); assert.equal(running, completed);
    const raw = await page.evaluate(() => [window.liveUiPreview.port.state.preferences.sourceFontPt, window.liveUiPreview.port.state.preferences.translationFontPt]); assert.deepEqual(raw, [sourceFontPt, translationFontPt]);
    assert.equal(await page.evaluate(() => window.liveUiPreview.port.commands.filter(({ command }) => ['set_hud_preference', 'save_scoped_preferences'].includes(command.kind)).length), 0);
    facts.push({ raw, running, completed });
  }
  await assertNoErrors(page, errors, 'manual-stream-and-complete-typography'); return facts;
});

