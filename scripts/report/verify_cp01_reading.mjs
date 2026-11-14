/** CP-01 production React journeys. Controlled transport is not native/game proof. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { createHash, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import nodeAssert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const desktop = path.join(root, 'apps/desktop');
const argument = name => process.argv[process.argv.indexOf(name) + 1];
const out = path.resolve(root, process.argv.includes('--output') ? argument('--output') : `.build/cp01-reading-20261104/qa-${Date.now()}`);
const nonce = process.argv.includes('--nonce') ? argument('--nonce') : randomUUID();
if (!out.startsWith(path.join(root, '.build') + path.sep) || fsSync.existsSync(out)) throw Error('Fresh repository .build output required');
await fs.mkdir(out, { recursive: true });
let assertionCount = 0;
const assert = new Proxy(nodeAssert, { apply(fn, self, args) { assertionCount++; return Reflect.apply(fn, self, args); }, get(fn, key) { const value = fn[key]; return typeof value === 'function' ? (...args) => { assertionCount++; return Reflect.apply(value, fn, args); } : value; } });
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const require = createRequire(path.join(root, 'package.json'));
const { chromium } = require('playwright-core');
const sampleScope = { window: {} };
vm.runInNewContext(await fs.readFile(path.join(root, 'docs/design/first-product-v1/data.js'), 'utf8'), sampleScope, { timeout: 2000 });
const sample = JSON.parse(JSON.stringify(sampleScope.window.LunaDesignData.manualSample));
async function files(directory) { const result = []; for (const item of await fs.readdir(directory, { withFileTypes: true })) { const file = path.join(directory, item.name); if (item.isDirectory()) result.push(...await files(file)); else if (item.isFile()) result.push(file); } return result.sort(); }
const audited = [...await files(path.join(desktop, 'src')), ...await files(path.join(desktop, 'tests')), fileURLToPath(import.meta.url)];
const sourceHashes = async () => Object.fromEntries(await Promise.all(audited.map(async file => [path.relative(root, file), sha(await fs.readFile(file))])));
const before = await sourceHashes();
const csp = JSON.parse(await fs.readFile(path.join(desktop, 'src-tauri/tauri.conf.json'), 'utf8')).app.security.csp;
const checks = [], captures = [], pageErrors = [], consoleErrors = [], externalRequests = [], builds = [];
const startedAt = new Date().toISOString();
let browser, server, origin;
async function runBuild(args, label) {
  const command = [process.execPath, ...args], child = spawn(command[0], command.slice(1), { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = ''; child.stdout.on('data', value => { stdout += value; }); child.stderr.on('data', value => { stderr += value; });
  const exitCode = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); });
  await fs.writeFile(path.join(out, label + '.json'), JSON.stringify({ command, exitCode, stdout, stderr }, null, 2) + '\n');
  builds.push({ command, exitCode, log: label + '.json' }); assert.equal(exitCode, 0, stderr);
}
async function check(id, execute) {
  const initial = assertionCount;
  try { const observations = await execute(); checks.push({ id, status: 'passed', assertionCount: assertionCount - initial, observations }); console.log(`${id}: passed`); }
  catch (error) { checks.push({ id, status: 'failed', assertionCount: assertionCount - initial, error: error.message, stack: error.stack }); console.log(`${id}: failed: ${error.stack ?? error}`); }
}
async function capture(page, name) {
  await page.evaluate(() => document.fonts.ready);
  const file = `${name}.png`; await page.screenshot({ path: path.join(out, file), animations: 'disabled' });
  captures.push({ file, sha256: sha(await fs.readFile(path.join(out, file))), viewport: page.viewportSize(), producer: 'production_react_controlled_transport', approvedReference: false, nativeVerified: false });
}
function initialize(port, sample, theme) {
  const unit = structuredClone(port.state.current), session = port.state.session.sessionId;
  if (unit.sourceText !== sample.source) throw Error('Existing typed fixture and approved sample differ');
  unit.translations = [{ translationId: 'cp01-result', taskId: 'cp01-completed-task', text: sample.translation, providerLabel: '受控翻译结果', targetLanguage: 'zh-CN', correction: null }];
  port.state.current = unit; port.state.history.units = [unit]; port.state.preferences.theme = theme;
  port.sessionRecordFixtures[`${session}:body`] = Array.from({ length: 140 }, (_, index) => ({ kind: 'body', sequence: String(index + 1), sourceRefs: [], unit: { ...structuredClone(unit), unitId: `cp01-body-${index + 1}`, tokens: [], sourceText: `本文 ${index + 1}`, sourceHash: String(index + 1).padStart(64, '0') } }));
  port.sessionRecordFixtures[`${session}:extraction`] = Array.from({ length: 140 }, (_, index) => ({ kind: 'extraction', sequence: String(index + 1), eventId: `cp01-raw-${index + 1}`, sourceId: 'ui-stream', wireSequence: String(index + 1), text: `抽出 ${index + 1}`, observedAtMs: unit.createdAtMs + index, reason: 'selected', bodyRefs: [], payloadRetained: true }));
  port.sessionRecordFixtures[`${session}:hook_events`] = Array.from({ length: 5 }, (_, index) => ({ kind: 'hook_event', sequence: String(index + 1), eventId: `cp01-hook-${index + 1}`, operationId: 'cp01-operation', sessionId: session, sourceId: null, wireSequence: null, observedAtMs: unit.createdAtMs + index, level: index === 1 ? 'warning' : 'info', stage: 'injection', code: 'core_log', message: `接入事实 ${index + 1}`, lostCount: null, integrity: 'intact' }));
  port.cp01Plans = []; port.cp01Pending = new Map();
  const original = port.sessionRecords.bind(port);
  port.sessionRecords = async query => {
    const value = await original(query);
    const index = port.cp01Plans.findIndex(plan => plan.dataset === query.dataset && (!plan.cursorOnly || query.cursor !== null));
    if (index >= 0) { const plan = port.cp01Plans.splice(index, 1)[0]; if (plan.hold) await new Promise(resolve => port.cp01Pending.set(plan.hold, resolve)); if (plan.error) throw Error(plan.error); }
    return value;
  };
}
async function app(theme = 'dark', viewport = { width: 1160, height: 900 }) {
  const p = await browser.newPage({ viewport, deviceScaleFactor: 1, colorScheme: theme }); p.setDefaultTimeout(10000);
  p.on('pageerror', error => pageErrors.push(error.message)); p.on('console', message => { if (message.type() === 'error') consoleErrors.push(message.text()); });
  await p.route('**/*', async route => { const url = new URL(route.request().url()); if (url.origin !== origin && !['data:', 'blob:'].includes(url.protocol)) { externalRequests.push(url.href); return route.abort(); } return route.continue(); });
  await p.addInitScript({ content: `window.__initializeControlledPort = port => (${initialize.toString()})(port, ${JSON.stringify(sample)}, ${JSON.stringify(theme)});` });
  const response = await p.goto(origin + '/tests/visual/index.html?role=main&scenario=reading'); assert.equal(await response.headerValue('content-security-policy'), csp);
  await p.waitForFunction(() => window.uiHarness && document.getElementById('root')?.children.length);
  // Normalize only the pre-existing test host, as in the maintained UI runner.
  // No production selector/style is patched for screenshot matching.
  await p.evaluate(() => Object.assign(document.getElementById('root').style, { width: '100%', height: '100%', left: '0', top: '0' }));
  await p.getByRole('navigation', { name: '主导航' }).getByRole('button', { name: '当前会话', exact: true }).click();
  await p.getByRole('article', { name: '当前原文与译文', exact: true }).waitFor();
  await p.waitForFunction(() => ['body', 'extraction', 'hook_events'].every(dataset => window.uiHarness.sessionRecordQueries.some(query => query.dataset === dataset)));
  return p;
}
const disclosure = (p, id) => p.locator(`details[data-flat-session-section="${id}"]`);
async function open(p, id) { const d = disclosure(p, id); if (await d.getAttribute('open') === null) await d.locator(':scope > summary').click(); await d.locator('[data-session-dataset]').waitFor(); return d; }
async function close(p, id) { const d = disclosure(p, id); if (await d.getAttribute('open') !== null) await d.locator(':scope > summary').click(); await p.waitForFunction(id => !document.querySelector(`details[data-flat-session-section="${id}"]`).open, id); }
const current = p => p.getByRole('article', { name: '当前原文与译文', exact: true });
const sourceText = locator => locator.evaluate(node => { const clone = node.cloneNode(true); clone.querySelectorAll('rt,rp').forEach(n => n.remove()); return clone.textContent; });
async function previewFrame(p) {
  await p.waitForFunction(() => window.uiHarness.previewAttaches.length === 1);
  await p.evaluate(async () => {
    const port = window.uiHarness, projection = [...port.previewProjections.values()].at(-1), canvas = document.createElement('canvas'); canvas.width = 160; canvas.height = 100;
    canvas.getContext('2d').fillRect(0, 0, 160, 100); const image = await new Promise(resolve => canvas.toBlob(resolve, 'image/jpeg'));
    port.emitPreview({ projection: { ...projection, state: 'live', capturedAtMs: '1792368001000' }, frame: { leaseId: projection.leaseId, generation: projection.generation, sequence: '1', targetIdentityHash: projection.targetIdentityHash, capturedAtMs: '1792368001000', width: 160, height: 100, contentHash: 'e'.repeat(64), encoding: 'jpeg', payload: [...new Uint8Array(await image.arrayBuffer())] } });
  });
  const preview = p.getByRole('region', { name: '运行窗口预览', exact: true }); await preview.locator('img').waitFor(); return preview;
}
try {
  await runBuild([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck-tests');
  const build = path.join(out, 'controlled-build');
  await runBuild([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(desktop, 'tests/visual/vite.config.ts'), '--outDir', build, '--sourcemap'], 'component-build');
  const maps = (await files(build)).filter(file => file.endsWith('.js.map')); const graph = (await Promise.all(maps.map(async file => JSON.parse(await fs.readFile(file, 'utf8')).sources))).flat();
  for (const source of ['ReadingPanel.tsx', 'SessionRecordPanel.tsx', 'SourceText.tsx']) assert.ok(graph.some(file => file.endsWith('/' + source)), `Actual production component in built graph: ${source}`);
  server = http.createServer(async (req, res) => {
    try { const file = path.resolve(build, '.' + new URL(req.url, 'http://localhost').pathname); if (!file.startsWith(build + path.sep)) throw Error('outside build'); const bytes = await fs.readFile(file); const extension = path.extname(file); res.writeHead(200, { 'content-type': ({ '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml' })[extension] ?? 'application/octet-stream', ...(extension === '.html' ? { 'content-security-policy': csp } : {}) }); res.end(bytes); } catch { res.writeHead(404); res.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disable-gpu', '--force-color-profile=srgb'] });
  await check('CP01-01-independent-current-and-default-collapsed-datasets', async () => {
    const p = await app(); try {
      assert.equal(await p.getByRole('tablist', { name: '当前会话内容', exact: true }).count(), 0);
      assert.equal(await current(p).evaluate(node => node.closest('details') === null), true);
      assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source); assert.equal(await current(p).getByText(sample.translation, { exact: true }).isVisible(), true);
      for (const id of ['reading', 'texts', 'logs']) { assert.equal(await disclosure(p, id).getAttribute('open'), null); assert.equal(await disclosure(p, id).locator('[data-session-dataset]').count(), 1, 'Collapsed datasets remain mounted'); }
      for (const id of ['reading', 'texts', 'logs']) await open(p, id);
      assert.equal(await p.locator('details[data-flat-session-section][open]').count(), 3);
      assert.equal(await current(p).isVisible(), true); await capture(p, 'three-independent-sections');
    } finally { await p.close(); }
  });
  await check('CP01-02-filter-page-scroll-and-follow-survive-collapse', async () => {
    const p = await app(); try {
      const body = await open(p, 'reading'), raw = await open(p, 'texts');
      await raw.getByRole('textbox', { name: '搜索会话原文', exact: true }).fill('抽出 139'); await raw.getByRole('log').getByText('抽出 139', { exact: true }).waitFor();
      await body.getByRole('button', { name: '加载较早记录', exact: true }).click(); await p.waitForFunction(() => document.querySelectorAll('[data-session-dataset="body"] [data-record-sequence]').length === 100);
      const list = body.getByRole('log'); await list.evaluate(node => { node.scrollTop = 220; });
      const before = await list.evaluate(node => ({ top: node.scrollTop, high: node.dataset.highWatermark, ids: [...node.querySelectorAll('[data-record-sequence]')].map(n => n.dataset.recordSequence) })); assert.ok(before.top > 0);
      await close(p, 'reading'); await open(p, 'reading');
      assert.deepEqual(await list.evaluate(node => ({ top: node.scrollTop, high: node.dataset.highWatermark, ids: [...node.querySelectorAll('[data-record-sequence]')].map(n => n.dataset.recordSequence) })), before);
      assert.equal(await body.getByRole('button', { name: '跟随新内容', exact: true }).count(), 0);
      assert.equal(await raw.getByRole('textbox', { name: '搜索会话原文', exact: true }).inputValue(), '抽出 139'); assert.equal(await raw.locator('[data-record-sequence]').count(), 1);
      await body.getByRole('textbox', { name: '搜索会话原文', exact: true }).fill('本文 139'); await body.getByRole('log').getByText('本文 139', { exact: true }).waitFor();
      await close(p, 'reading'); await open(p, 'reading'); assert.equal(await body.getByRole('textbox', { name: '搜索会话原文', exact: true }).inputValue(), '本文 139'); assert.equal(await raw.locator('[data-record-sequence]').count(), 1);
    } finally { await p.close(); }
  });
  await check('CP01-03-hidden-page-completion-preserves-anchor-and-errors', async () => {
    const results = [];
    for (const hiddenBy of ['collapse', 'main_navigation']) {
    const p = await app(); try {
      const body = await open(p, 'reading'), list = body.getByRole('log');
      await p.waitForFunction(() => document.querySelectorAll('[data-session-dataset="body"] [data-record-sequence]').length === 50);
      await list.evaluate(node => { node.scrollTop = 160; });
      const before = await list.evaluate(node => { const top = node.getBoundingClientRect().top, row = [...node.querySelectorAll('[data-record-sequence]')].find(n => n.getBoundingClientRect().bottom > top), offset = row.getBoundingClientRect().top - top; return { sequence: row.dataset.recordSequence, text: row.textContent, offset, rasterOffset: Math.round(offset * devicePixelRatio), highWatermark: node.dataset.highWatermark, viewport: { width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio } }; });
      await p.evaluate(() => window.uiHarness.cp01Plans.push({ dataset: 'body', cursorOnly: true, hold: 'cp01-page' }));
      await body.getByRole('button', { name: '加载较早记录', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.cp01Pending.has('cp01-page'));
      if (hiddenBy === 'collapse') await close(p, 'reading');
      else await p.getByRole('navigation', { name: '主导航' }).getByRole('button', { name: '游戏库', exact: true }).click();
      await p.evaluate(() => window.uiHarness.cp01Pending.get('cp01-page')()); await p.waitForFunction(() => document.querySelectorAll('[data-session-dataset="body"] [data-record-sequence]').length === 100);
      if (hiddenBy === 'collapse') await open(p, 'reading');
      else await p.getByRole('navigation', { name: '主导航' }).getByRole('button', { name: '当前会话', exact: true }).click();
      await p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const after = await list.evaluate(node => { const top = node.getBoundingClientRect().top, row = [...node.querySelectorAll('[data-record-sequence]')].find(n => n.getBoundingClientRect().bottom > top), offset = row.getBoundingClientRect().top - top; return { sequence: row.dataset.recordSequence, text: row.textContent, offset, rasterOffset: Math.round(offset * devicePixelRatio), highWatermark: node.dataset.highWatermark, viewport: { width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio } }; });
      // Independent review selected physical-raster position as the observable
      // scroll invariant. Keep raw subpixel geometry; no epsilon/PNG threshold
      // or product rounding is introduced to satisfy a floating-point equality.
      await fs.writeFile(path.join(out, `hidden-${hiddenBy}.geometry.json`), JSON.stringify({ before, after, rawOffsetDelta: after.offset - before.offset }, null, 2) + '\n');
      assert.deepEqual({ ...after, offset: undefined }, { ...before, offset: undefined }, `Hidden ${hiddenBy} completion preserves exact row/range/physical position`);
      assert.equal(await body.getByRole('textbox', { name: '搜索会话原文', exact: true }).inputValue(), '');
      assert.equal(await body.getByRole('button', { name: '跟随新内容', exact: true }).count(), 0);
      const scopeFacts = await p.evaluate(() => { const port = window.uiHarness, reads = port.sessionRecordQueries.filter(query => query.dataset === 'body'); return { freshReads: reads.filter(query => query.cursor === null).length, scopes: reads.filter(query => query.cursor !== null).map(query => { const scope = port.sessionRecordCursors.get(query.cursor); return { snapshotId: scope.snapshotId, highWatermark: scope.highWatermark, signature: scope.signature }; }) }; });
      assert.equal(scopeFacts.freshReads, 1); assert.ok(scopeFacts.scopes.length > 0); assert.ok(scopeFacts.scopes.every(scope => JSON.stringify(scope) === JSON.stringify(scopeFacts.scopes[0])));
      await p.evaluate(() => window.uiHarness.cp01Plans.push({ dataset: 'body', cursorOnly: true, error: 'CP01 explicit page failure' }));
      await body.getByRole('button', { name: '加载较早记录', exact: true }).click(); await body.getByRole('alert').waitFor(); await close(p, 'reading'); await open(p, 'reading');
      assert.equal(await body.getByRole('alert').count(), 1); assert.equal(await body.locator('[data-record-sequence]').count(), 100);
      await body.getByRole('button', { name: '重试', exact: true }).click(); await body.getByRole('alert').waitFor({ state: 'hidden' }); await p.waitForFunction(() => document.querySelectorAll('[data-session-dataset="body"] [data-record-sequence]').length === 140);
      results.push({ hiddenBy, anchorBefore: before, anchorAfter: after, rawOffsetDelta: after.offset - before.offset, measurement: 'Math.round(raw_css_offset * actual_device_scale_factor), exact physical-pixel equality', scopeFacts });
    } finally { await p.close(); }
    }
    return results;
  });
  await check('CP01-04-current-annotations-use-real-preference-commands', async () => {
    const p = await app(); try {
      const source = current(p).getByTestId('source-text'); assert.ok(await source.locator('rt').count() > 0); assert.ok(await source.getByRole('button').count() > 0);
      await p.getByRole('button', { name: '假名注音', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.state.preferences.showRuby === false); assert.equal(await source.locator('rt').count(), 0);
      await p.getByRole('button', { name: '可点击分词', exact: true }).click(); assert.equal(await source.getByRole('button').count(), 0); assert.equal(await sourceText(source), sample.source); assert.equal(await p.getByRole('button', { name: '词性提示', exact: true }).isDisabled(), true);
      await p.getByRole('button', { name: '假名注音', exact: true }).click(); assert.ok(await source.locator('rt').count() > 0, 'Ruby remains independent of clickable segmentation');
      await p.getByRole('button', { name: '可点击分词', exact: true }).click(); await p.getByRole('button', { name: '词性提示', exact: true }).click(); assert.equal(await source.getAttribute('data-pos'), 'false'); assert.equal(await sourceText(source), sample.source);
      const kinds = await p.evaluate(() => window.uiHarness.commands.map(command => command.command.kind)); assert.ok(kinds.includes('set_hud_preference')); assert.ok(kinds.includes('set_reading_annotations'));
    } finally { await p.close(); }
  });
  await check('CP01-05-old-word-card-survives-long-new-dialogue', async () => {
    const p = await app(); try {
      const old = await p.evaluate(() => structuredClone(window.uiHarness.state.current)); await current(p).getByTestId('source-text').getByRole('button').first().click();
      const word = p.getByRole('dialog', { name: '本地词典', exact: true }); await word.getByText('固定当前选词', { exact: true }).waitFor(); const frozen = await p.evaluate(() => structuredClone(window.uiHarness.wordCard));
      await p.evaluate(() => { const port = window.uiHarness, next = { ...structuredClone(port.state.current), unitId: 'cp01-next-long-unit', sourceHash: 'c'.repeat(64), sourceText: '新しい長い台詞。\n'.repeat(24), tokens: [], translations: [] }; port.publish({ commitRevision: port.state.commitRevision + 1, current: next, history: { units: [next, ...port.state.history.units], nextCursor: null } }); });
      await current(p).getByTestId('source-text').getByText(/新しい長い台詞/).waitFor(); assert.equal(await word.isVisible(), true);
      assert.deepEqual(await p.evaluate(() => ({ cardId: window.uiHarness.wordCard?.cardId, unitId: window.uiHarness.wordCard?.unit.unitId, textRevision: window.uiHarness.wordCard?.unit.textRevision, source: window.uiHarness.wordCard?.unit.sourceText })), { cardId: frozen.cardId, unitId: old.unitId, textRevision: old.textRevision, source: old.sourceText });
      await word.getByRole('button', { name: '打开学习窗口', exact: true }).click(); assert.equal(await p.evaluate(() => window.uiHarness.state.learningObject.unit.unitId), old.unitId); assert.equal(await p.evaluate(() => window.uiHarness.state.learningObject.unit.textRevision), old.textRevision); await capture(p, 'fixed-old-learning-long-new-current');
    } finally { await p.close(); }
  });
  await check('CP01-06-current-actions-and-preview-retain-ownership', async () => {
    const p = await app(); try {
      const original = await p.evaluate(() => structuredClone(window.uiHarness.state.current)); const preview = await previewFrame(p), image = await preview.locator('img').getAttribute('src');
      for (const id of ['reading', 'texts', 'logs']) { await open(p, id); await close(p, id); assert.equal(await preview.locator('img').getAttribute('src'), image); assert.equal(await p.evaluate(() => window.uiHarness.previewAttaches.length), 1); assert.equal(await p.evaluate(() => window.uiHarness.previewControls.filter(c => c.action === 'stop').length), 0); }
      await p.getByRole('button', { name: '复制原文', exact: true }).click(); assert.deepEqual(await p.evaluate(() => window.uiHarness.copiedReadingUnits.at(-1)), { unitId: original.unitId, textRevision: original.textRevision, text: original.sourceText });
      await p.getByRole('button', { name: '重新翻译本句', exact: true }).click(); const translation = await p.evaluate(() => window.uiHarness.retranslations.at(-1)); assert.equal(translation.unitId, original.unitId); assert.equal(translation.textRevision, original.textRevision); assert.ok(translation.operationId);
      await p.getByRole('button', { name: '分析本句文法', exact: true }).click(); assert.equal(await p.evaluate(() => window.uiHarness.state.learningObject.unit.unitId), original.unitId);
    } finally { await p.close(); }
  });
  await check('CP01-07-queued-running-stream-completed-no-generic-empty-flash', async () => {
    const p = await app(); try {
      for (const status of ['queued', 'running', 'completed']) {
        await p.evaluate(status => {
          const port = window.uiHarness, base = port.state.current, task = { taskId: `cp01-${status}`, unitId: base.unitId, textRevision: base.textRevision, purpose: 'translation', status, errorCode: null, partialText: status === 'running' ? '正在返回的译文片段' : null, providerLabel: 'CP01服务', attemptId: `cp01-attempt-${status}` };
          const unit = { ...base, translations: status === 'completed' ? [{ translationId: 'cp01-final', taskId: task.taskId, text: '最终译文', providerLabel: 'CP01服务', targetLanguage: 'zh-CN', correction: null }] : [] }, commitRevision = port.state.commitRevision + 1;
          port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = { profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit, translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines: [{ scopeKey: 'global', bindingId: 'cp01-binding', engineId: 'cp01-engine', providerLabel: 'CP01服务', status: { kind: 'current', task } }] } };
          port.publish({ commitRevision, current: unit, tasks: [task], history: { ...port.state.history, units: [unit] } });
        }, status);
        await current(p).getByText(status === 'queued' ? 'CP01服务 · 等待翻译' : status === 'running' ? 'CP01服务 · 翻译中' : '最终译文', { exact: true }).waitFor();
        assert.equal((await current(p).textContent()).includes('尚无译文'), false);
        if (status === 'running') assert.equal(await current(p).getByText('正在返回的译文片段', { exact: true }).isVisible(), true);
        assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source);
      }
    } finally { await p.close(); }
  });
  for (const variant of [{ id: 'dark', theme: 'dark', width: 1160, height: 900, zoom: 1 }, { id: 'light', theme: 'light', width: 1160, height: 900, zoom: 1 }, { id: 'narrow', theme: 'dark', width: 740, height: 1200, zoom: 1 }, { id: 'zoom200', theme: 'dark', width: 1160, height: 900, zoom: 2 }]) {
    await check('CP01-visual-' + variant.id, async () => {
      const p = await app(variant.theme, { width: variant.width, height: variant.height }); try {
        if (variant.zoom !== 1) await p.evaluate(zoom => { document.documentElement.style.zoom = String(zoom); }, variant.zoom);
        await p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        await current(p).scrollIntoViewIfNeeded();
        const measured = await p.evaluate(() => {
          const rect = node => [...node.getClientRects()].map(r => ({ x: r.x, y: r.y, left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height }));
          const card = document.querySelector('[data-guided-current]'), source = card.querySelector('[data-testid="source-text"]');
          return { theme: document.documentElement.dataset.theme, zoom: Number(getComputedStyle(document.documentElement).zoom), viewport: { width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio }, outer: [document.documentElement.scrollLeft, document.body.scrollLeft], current: rect(card)[0], source: rect(source)[0], tokens: [...source.querySelectorAll('[data-token-index],ruby,rt')].map(node => ({ tag: node.tagName, text: node.textContent, rects: rect(node) })), preview: rect(document.querySelector('aside [aria-label="运行窗口预览"]'))[0] };
        });
        await capture(p, 'current-reading-' + variant.id);
        await fs.writeFile(path.join(out, 'current-reading-' + variant.id + '.geometry.json'), JSON.stringify(measured, null, 2) + '\n');
        assert.equal(measured.theme, variant.theme); assert.equal(measured.zoom, variant.zoom); assert.deepEqual(measured.outer, [0, 0]); assert.equal(measured.viewport.deviceScaleFactor, 1); assert.equal(await p.locator('details[data-flat-session-section][open]').count(), 0);
        assert.equal(await current(p).isVisible(), true); assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source);
        const validRectangle = (rectangle, name) => {
          assert.ok(rectangle && ['left', 'right', 'top', 'bottom', 'width', 'height'].every(key => Number.isFinite(rectangle[key])), `${name} has actual finite DOM geometry`);
          assert.ok(rectangle.width > 0 && rectangle.height > 0, `${name} has positive visible area`);
          assert.equal(rectangle.width, rectangle.right - rectangle.left, `${name} width agrees exactly with its horizontal edges`);
          assert.equal(rectangle.height, rectangle.bottom - rectangle.top, `${name} height agrees exactly with its vertical edges`);
        };
        validRectangle(measured.current, 'Current card'); validRectangle(measured.source, 'SourceText'); validRectangle(measured.preview, 'Preview');
        const inWidth = rectangle => rectangle.left >= 0 && rectangle.right <= measured.viewport.width;
        const contained = (inner, outer) => inner.left >= outer.left && inner.right <= outer.right && inner.top >= outer.top && inner.bottom <= outer.bottom;
        const overlaps = (a, b) => Math.min(a.right, b.right) > Math.max(a.left, b.left) && Math.min(a.bottom, b.bottom) > Math.max(a.top, b.top);
        assert.ok(inWidth(measured.current), 'Current card stays in actual native-content width at every zoom');
        assert.ok(inWidth(measured.preview), 'Preview stays in actual native-content width at every zoom');
        assert.ok(contained(measured.source, measured.current), 'SourceText stays inside current card at every zoom');
        assert.equal(overlaps(measured.current, measured.preview), false, 'Preview must never cover the current card');
        assert.ok(measured.tokens.length > 0);
        for (const token of measured.tokens) {
          assert.ok(token.rects.length > 0, `Source token/ruby ${token.tag}:${token.text} exposes a visible rectangle`);
          for (const rectangle of token.rects) {
          validRectangle(rectangle, `Source token/ruby ${token.tag}:${token.text}`);
          assert.ok(contained(rectangle, measured.source), `Source token/ruby ${token.tag}:${token.text} stays within SourceText`);
          assert.equal(overlaps(rectangle, measured.preview), false, 'Preview does not cover source token or ruby');
        }
        }
        const nearby = p.locator('[aria-label="当前句操作"] button,[aria-label="阅读控制"] button,aside [aria-label="运行窗口预览"] button');
        const accessibility = [];
        assert.ok(await nearby.count() >= 10, 'All current and reading actions are present');
        const expectedActionNames = ['复制原文', '重新翻译本句', '可点击分词', '假名注音', '词性提示', '分析本句文法', '暂停自动输入', '关闭自动翻译', '重新选择文本流…', '查看历史', '刷新', '暂停', '预览详情'];
        const actionNames = await nearby.evaluateAll(nodes => nodes.map(node => node.getAttribute('aria-label') || node.textContent));
        assert.equal(new Set(actionNames).size, actionNames.length, 'Every nearby action has a unique accessible name');
        assert.deepEqual([...actionNames].sort(), [...expectedActionNames].sort(), 'The controlled scenario exposes the complete exact action set');
        for (let index = 0; index < await nearby.count(); index++) {
          const button = nearby.nth(index); await button.scrollIntoViewIfNeeded();
          const target = await button.evaluate(node => { const r = node.getBoundingClientRect(), x = r.left + r.width / 2, y = r.top + r.height / 2; return { name: node.getAttribute('aria-label') || node.textContent, left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height, viewportWidth: innerWidth, viewportHeight: innerHeight, hit: node.contains(document.elementFromPoint(x, y)) }; });
          accessibility.push(target);
          validRectangle(target, `Action ${target.name}`);
          assert.ok(target.width > 0 && target.height > 0 && target.left >= 0 && target.right <= target.viewportWidth && target.top >= 0 && target.bottom <= target.viewportHeight, `Action remains reachable after local scrolling: ${target.name}`);
          assert.equal(target.hit, true, `Action is not obscured: ${target.name}`);
        }
        await current(p).scrollIntoViewIfNeeded();
        await fs.writeFile(path.join(out, 'current-reading-' + variant.id + '.actions.json'), JSON.stringify(accessibility, null, 2) + '\n');
        return { ...measured, accessibility };

      } finally { await p.close(); }
    });
  }
} catch (error) { pageErrors.push(error.stack ?? String(error)); }
finally { await browser?.close(); if (server) await new Promise(resolve => server.close(resolve)); }
const after = await sourceHashes();
const errors = [...pageErrors, ...consoleErrors];
if (externalRequests.length) errors.push('Unexpected external network requests');
if (JSON.stringify(before) !== JSON.stringify(after)) errors.push('Audited product/test source changed during execution; rerun frozen sources');
const result = { schemaVersion: 1, id: 'cp01-current-reading-real-react', nonce, executedAt: startedAt, finishedAt: new Date().toISOString(), status: errors.length === 0 && checks.length === 11 && checks.every(check => check.status === 'passed') ? 'passed' : 'failed', layer: 'real_react_controlled_transport', assertionCount, skippedAssertions: 0, checks, captures, builds, sourceSha256: before, sourceSha256After: after, errors, externalRequests, nativeOrGameVerified: false, productReleaseApproved: false, pixelAcceptance: false, referenceApproval: 'not_granted' };
await fs.writeFile(path.join(out, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ status: result.status, checks: checks.length, assertionCount, captures: captures.length, errors, output: out }));
if (result.status !== 'passed') process.exitCode = 1;
