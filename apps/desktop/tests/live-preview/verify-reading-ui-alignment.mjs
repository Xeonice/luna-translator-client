/** Current Main UI regressions on real components, with isolated in-memory input. */
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
const output = path.join(root, '.build/current-client-execution-20261105/ui-difference-fixes/reading/run-' + Date.now());
const workspace = path.join(output, 'workspace');
const desktop = path.join(workspace, 'apps/desktop');
const results = [];
const tracked = ['apps/desktop/src/features/reading/ReadingPanel.tsx', 'apps/desktop/src/features/reading/styles.module.css', 'apps/desktop/src/features/reading/reading-typography.ts', 'apps/desktop/tests/live-preview/verify-reading-ui-alignment.mjs', 'apps/desktop/tests/live-preview/verify-live-preview.mjs'];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let browser, identity, server, origin, serverLog = '', fixedServerPid, sourceBefore;
const check = (name, run) => test(name, { timeout: 60000 }, async () => {
  try { const value = await run(); results.push({ name, status: 'passed', value }); }
  catch (error) { results.push({ name, status: 'failed', error: error.stack }); throw error; }
});
async function fixedHealth() {
  try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(500) })).json(); }
  catch { return null; }
}
async function record(name, value) { await fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n'); }
async function open(role = 'main', view = 'session', width = 1040, height = 720) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${origin}/client.html?role=${role}&view=${view}&state=normal&theme=dark`);
  await page.waitForFunction(() => Boolean(window.liveUiPreview?.port));
  if (role === 'main') await page.locator('nav [aria-current="page"]').filter({ hasText: '当前会话' }).waitFor();
  else await page.locator('#root > *').waitFor();
  return { page, errors };
}
// Prepare an explicit reading projection. This does not infer backend heads or run a model.
async function seed(page, options = {}) {
  return await page.evaluate(options => {
    const p = window.liveUiPreview.port;
    const base = structuredClone(p.state.current);
    const unit = { ...base, unitId: options.unitId ?? base.unitId, textRevision: options.textRevision ?? base.textRevision };
    if (options.sourceText) { unit.sourceText = options.sourceText; unit.tokens = []; unit.sourceHash = 'e'.repeat(64); }
    const providers = options.providers ?? ['受控译文甲'];
    const tasks = providers.map((providerLabel, index) => ({ taskId: `${unit.unitId}:task:${index}`, unitId: unit.unitId, textRevision: unit.textRevision, purpose: 'translation', status: options.phase ?? 'completed', errorCode: options.phase === 'failed' ? 'controlled_failure' : null, partialText: options.phase === 'running' ? options.text ?? '受控流式译文片段' : null, providerLabel, attemptId: `attempt:${unit.unitId}:${index}` }));
    unit.translations = options.phase === 'running' || options.phase === 'failed' ? [] : providers.map((providerLabel, index) => ({ translationId: `${unit.unitId}:translation:${index}`, taskId: tasks[index].taskId, text: options.text ?? `受控译文内容${index + 1}`, providerLabel, targetLanguage: index === 0 ? 'zh-CN' : 'en', correction: options.corrected ? { revision: 1, text: '保留的人工修正文' } : null }));
    const commitRevision = p.state.commitRevision + 1;
    const projection = { profileId: p.state.profileId, serverInstanceId: p.state.serverInstanceId, profileGeneration: p.state.profileGeneration, commitRevision, unit: structuredClone(unit), translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines: tasks.map((task, index) => ({ scopeKey: 'global', bindingId: `binding:${index}`, engineId: `engine:${index}`, providerLabel: task.providerLabel, status: { kind: 'current', task } })) } };
    const key = JSON.stringify([unit.unitId, unit.textRevision]);
    p.readingProjectionFixtures[key] = projection;
    p.readingUnitFixtures[key] = structuredClone(unit);
    p.publish({ current: unit, history: { units: [unit, ...p.state.history.units.filter(value => value.unitId !== unit.unitId)], nextCursor: null }, commitRevision, tasks, ...(options.preferences ? { preferences: { ...p.state.preferences, ...options.preferences } } : {}) });
    return { unitId: unit.unitId, textRevision: unit.textRevision, sourceText: unit.sourceText, commitRevision };
  }, options);
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
  await record('results', { kind: 'real-production-reading-ui-only', browser: identity, sourceBefore, sourceAfter, fixedServerPid, fixedAfter, cases: results, native: 'not_run', product_completion: 'incomplete' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  assert.equal(fixedAfter, fixedServerPid, 'The user preview process must not change');
  assert.deepEqual(sourceAfter, sourceBefore, 'Production source must remain stable during the isolated run');
  console.log('Reading UI evidence: ' + output);
});

check('Main keeps the seven frozen-object daily actions in approved order', async () => {
  const { page, errors } = await open(); const unit = await seed(page);
  const actions = page.getByRole('group', { name: '当前句操作', exact: true });
  assert.deepEqual(await actions.getByRole('button').allTextContents(), ['复制原文', '重新翻译', '暂停自动输入', '关闭自动翻译', '重新选择文本流…', '分析当前句', '查看历史']);
  await actions.getByRole('button', { name: '复制原文', exact: true }).click();
  await page.waitForFunction(() => window.liveUiPreview.port.copiedReadingUnits.length === 1);
  assert.deepEqual(await page.evaluate(() => window.liveUiPreview.port.copiedReadingUnits[0]), unit && { unitId: unit.unitId, textRevision: unit.textRevision, text: unit.sourceText });
  await actions.getByRole('button', { name: '重新翻译本句', exact: true }).click();
  await page.waitForFunction(() => window.liveUiPreview.port.retranslations.length === 1);
  assert.deepEqual(await page.evaluate(() => { const r = window.liveUiPreview.port.retranslations[0]; return [r.unitId, r.textRevision, typeof r.operationId]; }), [unit.unitId, unit.textRevision, 'string']);
  await actions.getByRole('button', { name: '暂停自动输入', exact: true }).click(); await actions.getByRole('button', { name: '继续自动输入', exact: true }).waitFor();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.state.session.paused), true);
  await actions.getByRole('button', { name: '继续自动输入', exact: true }).click(); await actions.getByRole('button', { name: '暂停自动输入', exact: true }).waitFor();
  await actions.getByRole('button', { name: '关闭自动翻译', exact: true }).click(); await actions.getByRole('button', { name: '启用自动翻译', exact: true }).waitFor();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.state.session.translationEnabled), false);
  await actions.getByRole('button', { name: '启用自动翻译', exact: true }).click();
  await actions.getByRole('button', { name: '重新选择文本流…', exact: true }).click(); await page.getByRole('dialog').waitFor();
  await page.keyboard.press('Escape'); await page.getByRole('dialog').waitFor({ state: 'hidden' });
  await actions.getByRole('button', { name: '分析本句文法', exact: true }).click();
  await page.waitForFunction(() => Boolean(window.liveUiPreview.port.state.learningObject));
  assert.deepEqual(await page.evaluate(() => { const u = window.liveUiPreview.port.state.learningObject.unit; return [u.unitId, u.textRevision]; }), [unit.unitId, unit.textRevision]);
  await actions.getByRole('button', { name: '查看历史', exact: true }).click();
  await page.locator('nav [aria-current="page"]').filter({ hasText: '历史记录' }).waitFor();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.mainPageRequests.at(-1).intent), 'history');
  await assertNoErrors(page, errors, 'daily-actions-history'); return { unit, actions: 7 };
});

check('Main removes only the duplicate annotation control row', async () => {
  const { page, errors } = await open(); await seed(page);
  assert.equal(await page.getByRole('group', { name: '阅读控制', exact: true }).count(), 0);
  for (const name of ['可点击分词', '假名注音', '词性提示']) assert.equal(await page.getByRole('button', { name, exact: true }).count(), 0);
  assert.equal(await page.getByRole('group', { name: '当前句操作', exact: true }).getByRole('button').count(), 7);
  await assertNoErrors(page, errors, 'main-no-duplicate-preference-row');
});

check('single and corrected translations have one footer provenance while multiple results keep individual labels', async () => {
  const { page, errors } = await open(); const card = page.getByRole('article', { name: '当前原文与译文', exact: true });
  await seed(page, { providers: ['受控译文甲'] }); await card.getByText('受控译文内容1', { exact: true }).waitFor();
  assert.equal(await card.locator('[class*="translation"] small').count(), 0);
  assert.match(await card.locator('footer').innerText(), /受控译文甲.*zh-CN/);
  assert.equal((await card.innerText()).split('受控译文甲').length - 1, 1);
  await page.screenshot({ path: path.join(output, 'provenance-single.png') });
  await seed(page, { providers: ['受控译文甲'], corrected: true }); await card.getByText('保留的人工修正文', { exact: true }).waitFor();
  assert.equal(await card.locator('[class*="translation"] small').count(), 0);
  assert.match(await card.locator('footer').innerText(), /受控译文甲.*zh-CN.*人工修正/);
  await page.screenshot({ path: path.join(output, 'provenance-corrected.png') });
  await seed(page, { providers: ['受控译文甲', '受控译文乙'] }); await card.getByText('受控译文内容2', { exact: true }).waitFor();
  const labels = await card.locator('[class*="translation"] small').allTextContents();
  assert.equal(labels.length, 2); assert.match(labels[0], /受控译文甲.*zh-CN/); assert.match(labels[1], /受控译文乙.*en/);
  await page.screenshot({ path: path.join(output, 'provenance-multiple.png') });
  await seed(page, { providers: [''] }); await card.getByText('受控译文内容1', { exact: true }).waitFor();
  assert.match(await card.locator('[class*="translation"] small').innerText(), /zh-CN/);
  await assertNoErrors(page, errors, 'provenance-single-and-multiple');
});

check('Main typography derives from untouched snapshot preferences for default and custom values', async () => {
  const { page, errors } = await open(); const actual = [];
  for (const [sourceFontPt, translationFontPt] of [[18, 24], [29, 35]]) {
    await seed(page, { preferences: { sourceFontPt, translationFontPt } });
    await page.getByRole('article', { name: '当前原文与译文', exact: true }).getByText('受控译文内容1', { exact: true }).waitFor();
    const facts = await page.evaluate(() => { const card = document.querySelector('[data-guided-current]'); return { source: parseFloat(getComputedStyle(card.querySelector('[data-testid="source-text"]')).fontSize), translation: parseFloat(getComputedStyle(card.querySelector('[class*="translations"] article p')).fontSize), raw: [window.liveUiPreview.port.state.preferences.sourceFontPt, window.liveUiPreview.port.state.preferences.translationFontPt] }; });
    assert.ok(Math.abs(facts.source - sourceFontPt * 23 / 18) < .001); assert.ok(Math.abs(facts.translation - translationFontPt * 18 / 24) < .001); assert.deepEqual(facts.raw, [sourceFontPt, translationFontPt]); actual.push(facts);
    assert.equal(await page.evaluate(() => window.liveUiPreview.port.commands.filter(({ command }) => ['set_reading_annotations', 'set_hud_preference', 'save_scoped_preferences'].includes(command.kind)).length), 0);
    await page.screenshot({ path: path.join(output, `typography-main-${sourceFontPt}-${translationFontPt}.png`) });
  }
  await assertNoErrors(page, errors, 'typography-default-and-custom'); return actual;
});

check('Main streaming and completed text keep the same derived typography', async () => {
  const { page, errors } = await open(); const facts = [];
  for (const translationFontPt of [24, 35]) {
    await seed(page, { phase: 'running', text: '受控流式译文片段', preferences: { translationFontPt } });
    const card = page.getByRole('article', { name: '当前原文与译文', exact: true });
    const partial = card.getByText('受控流式译文片段', { exact: true }); await partial.waitFor();
    const running = await partial.evaluate(node => parseFloat(getComputedStyle(node).fontSize));
    await seed(page, { text: '受控完整译文内容', preferences: { translationFontPt } });
    const completed = card.getByText('受控完整译文内容', { exact: true }); await completed.waitFor();
    const done = await completed.evaluate(node => parseFloat(getComputedStyle(node).fontSize));
    assert.ok(Math.abs(running - done) < .001); assert.ok(Math.abs(done - translationFontPt * 18 / 24) < .001); facts.push({ translationFontPt, running, completed: done });
  }
  await assertNoErrors(page, errors, 'typography-stream-complete'); return facts;
});

check('HUD and Settings retain all three annotation preferences and their real command paths', async () => {
  const hud = await open('hud', 'hud', 560, 290);
  const hudFonts = await hud.page.evaluate(() => ({ source: parseFloat(getComputedStyle(document.querySelector('[data-testid="source-text"]')).fontSize), raw: window.liveUiPreview.port.state.preferences.sourceFontPt }));
  assert.equal(hudFonts.source, hudFonts.raw, 'HUD retains its own unscaled preference');
  for (const label of ['词性提示', '假名注音', '可点击分词']) { await hud.page.getByRole('button', { name: label, exact: true }).click(); await hud.page.getByRole('button', { name: label, exact: true }).getAttribute('aria-pressed').then(value => assert.equal(value, 'false')); }
  const hudFacts = await hud.page.evaluate(() => ({ values: [window.liveUiPreview.port.state.preferences.segmentationEnabled, window.liveUiPreview.port.state.preferences.showRuby, window.liveUiPreview.port.state.preferences.showPos], commands: window.liveUiPreview.port.commands.map(value => value.command.kind) }));
  assert.deepEqual(hudFacts.values, [false, false, false]); assert.ok(hudFacts.commands.includes('set_hud_preference')); assert.ok(hudFacts.commands.includes('set_reading_annotations'));
  await assertNoErrors(hud.page, hud.errors, 'hud-annotations-retained');
  const settings = await open('settings', 'settings-general', 1000, 720);
  // Edit POS before segmentation disables it; all original value/save assertions remain.
  for (const label of ['显示词性标记', '显示日语注音', '可点击日语分词']) { const control = settings.page.getByRole('switch', { name: label, exact: true }); await control.waitFor(); await control.focus(); await settings.page.keyboard.press('Space'); assert.equal(await control.isChecked(), false); }
  assert.equal(await settings.page.getByRole('switch', { name: '显示词性标记', exact: true }).isDisabled(), true);
  await settings.page.getByRole('button', { name: '保存阅读偏好', exact: true }).click();
  await settings.page.waitForFunction(() => window.liveUiPreview.port.commands.some(value => value.command.kind === 'save_scoped_preferences'));
  const saved = await settings.page.evaluate(() => window.liveUiPreview.port.commands.find(value => value.command.kind === 'save_scoped_preferences').command.payload.preferences);
  assert.deepEqual([saved.segmentationEnabled, saved.showRuby, saved.showPos], [false, false, false]); assert.equal(saved.scopeKey, 'global');
  await assertNoErrors(settings.page, settings.errors, 'settings-annotations-retained'); return { hudFacts, savedValues: [saved.segmentationEnabled, saved.showRuby, saved.showPos] };
});

check('async reading and translation preserve user scroll and frozen study ownership', async () => {
  const { page, errors } = await open(); const old = await seed(page);
  const token = page.locator('[data-guided-current] [data-testid="source-text"] button').first(); await token.click();
  await page.waitForFunction(() => Boolean(window.liveUiPreview.port.wordCard));
  await page.getByRole('group', { name: '当前句操作' }).getByRole('button', { name: '分析本句文法', exact: true }).click();
  await page.waitForFunction(() => Boolean(window.liveUiPreview.port.state.learningObject));
  // Close by its explicit owner before scrolling; the word-card policy itself is unchanged.
  await page.keyboard.press('Escape');
  const long = '这是用于检验真实滚动保留的长译文。'.repeat(180);
  const card = page.getByRole('article', { name: '当前原文与译文', exact: true });
  await seed(page, { text: long }); await card.getByText(long, { exact: true }).waitFor();
  const before = await page.locator('[class*="sessionContent"]').evaluate(node => { node.scrollTop = 90; return node.scrollTop; }); assert.equal(before, 90);
  await seed(page, { text: long.replace('这是', '这里') });
  await card.getByText(long.replace('这是', '这里'), { exact: true }).waitFor();
  const afterTranslation = await page.locator('[class*="sessionContent"]').evaluate(node => node.scrollTop); assert.equal(afterTranslation, before);
  await seed(page, { unitId: 'ui-next-sentence', sourceText: '新しい台詞が届いた。', text: long });
  await page.locator('[data-guided-current] [data-testid="source-text"]').filter({ hasText: '新しい台詞が届いた。' }).waitFor();
  const afterSource = await page.locator('[class*="sessionContent"]').evaluate(node => node.scrollTop); assert.equal(afterSource, before);
  const frozen = await page.evaluate(() => ({ learning: [window.liveUiPreview.port.state.learningObject.unit.unitId, window.liveUiPreview.port.state.learningObject.unit.textRevision], source: window.liveUiPreview.port.state.learningObject.unit.sourceText, main: window.liveUiPreview.port.state.current.unitId, documentScroll: document.scrollingElement.scrollTop }));
  assert.deepEqual(frozen.learning, [old.unitId, old.textRevision]); assert.equal(frozen.source, old.sourceText); assert.equal(frozen.main, 'ui-next-sentence'); assert.equal(frozen.documentScroll, 0);
  await assertNoErrors(page, errors, 'async-scroll-and-fixed-study'); return { before, afterTranslation, afterSource, frozen };
});

check('a new sentence does not reassign the open word request to new text', async () => {
  const { page, errors } = await open(); const old = await seed(page);
  await page.locator('[data-guided-current] [data-testid="source-text"] button').first().click();
  await page.waitForFunction(() => Boolean(window.liveUiPreview.port.wordCard));
  const before = await page.evaluate(() => structuredClone(window.liveUiPreview.port.wordCard));
  await seed(page, { unitId: 'ui-word-new-sentence', sourceText: '別の文章を読んでいる。', text: '后续一句译文' });
  await page.locator('[data-guided-current] [data-testid="source-text"]').filter({ hasText: '別の文章を読んでいる。' }).waitFor();
  const after = await page.evaluate(() => structuredClone(window.liveUiPreview.port.wordCard));
  assert.equal(after.cardId, before.cardId); assert.equal(after.unit.unitId, old.unitId); assert.equal(after.unit.textRevision, old.textRevision); assert.equal(after.unit.sourceText, old.sourceText);
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.wordCardRequests.length), 1);
  await assertNoErrors(page, errors, 'word-request-keeps-old-sentence'); return { cardId: after.cardId, unitId: after.unit.unitId, nativeSurface: 'not_run' };
});

check('Main retains real failed and absent-task facts instead of inventing success', async () => {
  const { page, errors } = await open(); const card = page.getByRole('article', { name: '当前原文与译文', exact: true });
  await seed(page, { phase: 'failed' }); await card.getByText('受控译文甲 · 翻译失败：controlled_failure', { exact: true }).waitFor();
  assert.match(await card.locator('footer').innerText(), /部分翻译未完成/);
  assert.equal(await card.getByText('已完成', { exact: true }).count(), 0);
  await seed(page, { providers: [] }); await card.getByText('当前原句暂无翻译任务。', { exact: true }).waitFor();
  assert.match(await card.locator('footer').innerText(), /当前原句/);
  await assertNoErrors(page, errors, 'failed-and-absent-task-facts');
});
