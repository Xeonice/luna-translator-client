/** Current-reading composition, using production React and the typed controlled
 * ApplicationPort. These checks do not attest native preview, SQLite or games. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { launchLockedBrowser, verifyBrowserIdentity } from './locked-browser.mjs';
import { createHash, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const desktop = path.join(root, 'apps/desktop');
const outputArgument = process.argv.indexOf('--output');
const out = path.resolve(root, outputArgument >= 0 ? process.argv[outputArgument + 1] : `.build/current-client-execution-20261105/reading/run-${Date.now()}`);
if (!out.startsWith(path.join(root, '.build') + path.sep) || fsSync.existsSync(out)) throw Error('A fresh repository .build directory is required');
await fs.mkdir(out, { recursive: true });
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const owned = ['apps/desktop/src/features/reading/ReadingPanel.tsx', 'apps/desktop/src/features/reading/styles.module.css', path.relative(root, fileURLToPath(import.meta.url)), 'apps/desktop/tests/visual/locked-browser.mjs'];
const hashes = async () => Object.fromEntries(await Promise.all(owned.map(async file => [file, sha(await fs.readFile(path.join(root, file)))])));
const before = await hashes(), startedAt = new Date().toISOString();
const csp = JSON.parse(await fs.readFile(path.join(desktop, 'src-tauri/tauri.conf.json'), 'utf8')).app.security.csp;
const sampleScope = { window: {} };
vm.runInNewContext(await fs.readFile(path.join(root, 'docs/design/first-product-v1/data.js'), 'utf8'), sampleScope, { timeout: 2000 });
const sample = JSON.parse(JSON.stringify(sampleScope.window.LunaDesignData.manualSample));
const checks = [], captures = [], builds = [], errors = [], pageErrors = [], consoleErrors = [], externalRequests = [];
let browser, browserIdentity, server, origin;
async function files(directory) { const entries = []; for (const entry of await fs.readdir(directory, { withFileTypes: true })) { const file = path.join(directory, entry.name); if (entry.isDirectory()) entries.push(...await files(file)); else entries.push(file); } return entries.sort(); }
async function build(args, label) {
  const command = [process.execPath, ...args], child = spawn(command[0], command.slice(1), { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = ''; child.stdout.on('data', bytes => { stdout += bytes; }); child.stderr.on('data', bytes => { stderr += bytes; });
  const exitCode = await new Promise((resolve, reject) => { child.once('error', reject); child.once('close', resolve); });
  await fs.writeFile(path.join(out, label + '.json'), JSON.stringify({ command, exitCode, stdout, stderr }, null, 2) + '\n');
  builds.push({ command, exitCode, log: label + '.json' }); assert.equal(exitCode, 0, stderr);
}
async function check(id, execute) {
  try { const observations = await execute(); checks.push({ id, status: 'passed', observations }); console.log(`${id}: passed`); }
  catch (error) { checks.push({ id, status: 'failed', error: error.message, stack: error.stack }); console.error(`${id}: failed: ${error.stack ?? error}`); }
}
async function capture(page, name) {
  await page.evaluate(() => document.fonts.ready);
  const file = name + '.png'; await page.screenshot({ path: path.join(out, file), animations: 'disabled' });
  captures.push({ file, sha256: sha(await fs.readFile(path.join(out, file))), viewport: page.viewportSize(), producer: 'production_react_controlled_transport', approvedReference: false });
}
// The test host controls transport responses only. It does not insert product
// markup, compute business projections or patch production component styles.
function initialize(port, sample, mode) {
  const unit = structuredClone(port.state.current);
  if (unit.sourceText !== sample.source) throw Error('Typed reading fixture and declared sample disagree');
  unit.translations = [{ translationId: 'reading-layout-result', taskId: 'reading-layout-completed', text: sample.translation, providerLabel: '受控翻译服务', targetLanguage: 'zh-CN', correction: null }];
  port.state.current = unit; port.state.history.units = [unit];
  if (mode === 'empty') { port.state.current = null; port.state.history.units = []; }
  if (mode === 'stopped') port.state.session.state = 'stopped';
  port.layoutCopyHolds = new Map();
  const copy = port.copyReadingUnit.bind(port);
  port.copyReadingUnit = async (unitId, revision) => { await copy(unitId, revision); if (port.layoutHoldCopy) await new Promise(resolve => port.layoutCopyHolds.set('copy', resolve)); };
}
const current = p => p.getByRole('article', { name: '当前原文与译文', exact: true });
const actions = p => p.getByRole('group', { name: '当前句操作', exact: true });
const options = p => p.getByRole('group', { name: '阅读控制', exact: true });
const sourceText = locator => locator.evaluate(node => { const clone = node.cloneNode(true); clone.querySelectorAll('rt,rp').forEach(item => item.remove()); return clone.textContent; });
async function app({ mode = 'normal', width = 1040, height = 900 } = {}) {
  const p = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1, colorScheme: 'dark' }); p.setDefaultTimeout(10000);
  p.on('pageerror', error => pageErrors.push(error.message)); p.on('console', message => { if (message.type() === 'error') consoleErrors.push(message.text()); });
  await p.route('**/*', async route => { const url = new URL(route.request().url()); if (url.origin !== origin && !['data:', 'blob:'].includes(url.protocol)) { externalRequests.push(url.href); return route.abort(); } return route.continue(); });
  await p.addInitScript({ content: `window.__initializeControlledPort = port => (${initialize.toString()})(port, ${JSON.stringify(sample)}, ${JSON.stringify(mode)});` });
  const response = await p.goto(origin + '/tests/visual/index.html?role=main&scenario=reading'); assert.equal(await response.headerValue('content-security-policy'), csp);
  await p.waitForFunction(() => window.uiHarness && document.getElementById('root')?.children.length);
  await p.evaluate(() => Object.assign(document.getElementById('root').style, { width: '100%', height: '100%', left: '0', top: '0' }));
  await p.getByRole('navigation', { name: '主导航' }).getByRole('button', { name: '当前会话', exact: true }).click();
  await actions(p).waitFor(); if (mode !== 'empty') await current(p).waitFor();
  await p.waitForFunction(() => ['body', 'extraction', 'hook_events'].every(dataset => window.uiHarness.sessionRecordQueries.some(query => query.dataset === dataset)));
  return p;
}
async function publishTranslation(p, status) {
  await p.evaluate(status => {
    const port = window.uiHarness, base = port.state.current, task = { taskId: `layout-${status}`, unitId: base.unitId, textRevision: base.textRevision, purpose: 'translation', status, errorCode: status === 'failed' ? 'SERVICE_UNAVAILABLE' : null, partialText: status === 'running' ? '正在返回的译文片段' : null, providerLabel: '受控翻译服务', attemptId: `layout-attempt-${status}` };
    const unit = { ...base, translations: status === 'completed' ? [{ translationId: 'layout-completed', taskId: task.taskId, text: '最终译文', providerLabel: '受控翻译服务', targetLanguage: 'zh-CN', correction: null }] : [] }, commitRevision = port.state.commitRevision + 1;
    port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = { profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit, translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines: [{ scopeKey: 'global', bindingId: 'layout-binding', engineId: 'layout-engine', providerLabel: '受控翻译服务', status: { kind: 'current', task } }] } };
    port.publish({ commitRevision, current: unit, tasks: [task], history: { ...port.state.history, units: [unit] } });
  }, status);
}
try {
  await build([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck-tests');
  const built = path.join(out, 'controlled-build');
  await build([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(desktop, 'tests/visual/vite.config.ts'), '--outDir', built, '--sourcemap'], 'component-build');
  const graph = (await Promise.all((await files(built)).filter(file => file.endsWith('.js.map')).map(async file => JSON.parse(await fs.readFile(file, 'utf8')).sources))).flat();
  for (const component of ['ReadingPanel.tsx', 'SessionRecordPanel.tsx', 'SourceText.tsx', 'WordCard.tsx', 'WindowPreview.tsx']) assert.ok(graph.some(file => file.endsWith('/' + component)), `Built production component: ${component}`);
  await fs.writeFile(path.join(out, 'production-source-graph.json'), JSON.stringify(graph, null, 2) + '\n');
  server = http.createServer(async (req, res) => {
    try { const file = path.resolve(built, '.' + new URL(req.url, 'http://localhost').pathname); if (!file.startsWith(built + path.sep)) throw Error('outside build'); const bytes = await fs.readFile(file), extension = path.extname(file); res.writeHead(200, { 'content-type': ({ '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml' })[extension] ?? 'application/octet-stream', ...(extension === '.html' ? { 'content-security-policy': csp } : {}) }); res.end(bytes); } catch { res.writeHead(404); res.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());
  await check('CP01A-current-first-seven-actions-and-collapsed-records', async () => {
    const p = await app(); try {
      assert.equal(await current(p).evaluate(node => node.closest('details') === null), true);
      assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source);
      await current(p).getByText(sample.translation, { exact: true }).waitFor();
      assert.deepEqual(await actions(p).getByRole('button').allTextContents(), ['复制原文', '重新翻译', '暂停自动输入', '关闭自动翻译', '重新选择文本流…', '分析当前句', '查看历史']);
      assert.equal(await actions(p).evaluate(node => !node.closest('article') && Boolean(node.previousElementSibling?.matches('article[data-guided-current]'))), true);
      assert.equal(await current(p).getByRole('button', { name: '复制原文', exact: true }).count(), 0);
      assert.equal(await options(p).getByRole('button').count(), 3);
      for (const id of ['reading', 'texts', 'logs']) { const d = p.locator(`details[data-flat-session-section="${id}"]`); assert.equal(await d.getAttribute('open'), null); assert.equal(await d.locator('[data-session-dataset]').count(), 1); assert.equal(await d.evaluate(node => Boolean(document.querySelector('[data-guided-current]').compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING)), true); }
      await capture(p, 'current-1040-normal');
    } finally { await p.close(); }
  });
  await check('CP01A-real-port-copy-retranslate-learning-and-history', async () => {
    const p = await app(); try {
      const identity = await p.evaluate(() => ({ unitId: window.uiHarness.state.current.unitId, textRevision: window.uiHarness.state.current.textRevision }));
      await actions(p).getByRole('button', { name: '复制原文', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.copiedReadingUnits.length === 1);
      assert.deepEqual(await p.evaluate(() => window.uiHarness.copiedReadingUnits[0]), { ...identity, text: sample.source });
      await actions(p).getByRole('button', { name: '重新翻译本句', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.retranslations.length === 1);
      const retry = await p.evaluate(() => window.uiHarness.retranslations[0]); assert.deepEqual({ unitId: retry.unitId, textRevision: retry.textRevision }, identity); assert.match(retry.operationId, /^[0-9a-f-]{36}$/);
      await actions(p).getByRole('button', { name: '分析本句文法', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.state.learningObject !== null);
      assert.deepEqual(await p.evaluate(() => ({ unitId: window.uiHarness.state.learningObject.unit.unitId, textRevision: window.uiHarness.state.learningObject.unit.textRevision })), identity);
      assert.equal(await p.evaluate(() => window.uiHarness.state.learningObject.tokenIndex), null); assert.equal(await p.evaluate(() => window.uiHarness.studies.length), 0, 'Opening analysis does not automatically call a model');
      await actions(p).getByRole('button', { name: '查看历史', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.mainPageRequests.at(-1)?.intent === 'history');
    } finally { await p.close(); }
  });
  await check('CP01A-capture-display-and-navigation-independent-of-sentence-request', async () => {
    const p = await app(); try {
      await p.evaluate(() => { window.uiHarness.layoutHoldCopy = true; });
      await actions(p).getByRole('button', { name: '复制原文', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.layoutCopyHolds.has('copy'));
      assert.equal(await actions(p).getByRole('button', { name: '重新翻译本句', exact: true }).isDisabled(), true);
      assert.equal(await actions(p).getByRole('button', { name: '暂停自动输入', exact: true }).isEnabled(), true);
      assert.equal(await options(p).getByRole('button', { name: '假名注音', exact: true }).isEnabled(), true);
      await actions(p).getByRole('button', { name: '暂停自动输入', exact: true }).click(); await actions(p).getByRole('button', { name: '继续自动输入', exact: true }).waitFor();
      await actions(p).getByRole('button', { name: '关闭自动翻译', exact: true }).click(); await actions(p).getByRole('button', { name: '启用自动翻译', exact: true }).waitFor();
      assert.deepEqual(await p.evaluate(() => window.uiHarness.commands.filter(item => ['set_capture_paused', 'set_translation_enabled'].includes(item.command.kind)).map(item => item.command)), [{ kind: 'set_capture_paused', payload: { paused: true } }, { kind: 'set_translation_enabled', payload: { enabled: false } }]);
      assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source);
      await p.evaluate(() => window.uiHarness.layoutCopyHolds.get('copy')()); await p.waitForFunction(() => !document.querySelector('[aria-label="当前句操作"] button').disabled);
      await actions(p).getByRole('button', { name: '重新选择文本流…', exact: true }).click();
      const sheet = p.getByRole('dialog', { name: '选择正文来源', exact: true }); await sheet.waitFor();
      await sheet.getByRole('button').filter({ hasText: 'UI fixture text stream' }).click();
      await p.waitForFunction(() => window.uiHarness.commands.some(item => item.command.kind === 'select_source' && item.command.payload.sourceId === 'ui-stream'));
    } finally { await p.close(); }
  });
  await check('CP01A-empty-and-disconnected-command-boundaries', async () => {
    for (const mode of ['empty', 'stopped']) {
      const p = await app({ mode }); try {
        for (const label of ['复制原文', '重新翻译本句', '分析本句文法']) assert.equal(await actions(p).getByRole('button', { name: label, exact: true }).isDisabled(), mode === 'empty');
        for (const label of ['暂停自动输入', '关闭自动翻译', '重新选择文本流…']) assert.equal(await actions(p).getByRole('button', { name: label, exact: true }).isDisabled(), mode === 'stopped');
        assert.equal(await actions(p).getByRole('button', { name: '查看历史', exact: true }).isEnabled(), true);
        if (mode === 'empty') { assert.equal(await current(p).count(), 0); await p.getByRole('heading', { name: '等待游戏原文', exact: true }).waitFor(); }
        else { await actions(p).getByRole('button', { name: '复制原文', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.copiedReadingUnits.length === 1); }
        await capture(p, mode);
      } finally { await p.close(); }
    }
  });
  await check('CP01A-translation-status-source-and-failure-remain-readable', async () => {
    const p = await app(); try {
      for (const status of ['queued', 'running', 'failed', 'cancelled', 'completed']) {
        await publishTranslation(p, status);
        const label = { queued: '受控翻译服务 · 等待翻译', running: '受控翻译服务 · 翻译中', failed: '受控翻译服务 · 翻译失败：SERVICE_UNAVAILABLE', cancelled: '受控翻译服务 · 翻译已取消', completed: '最终译文' }[status];
        await current(p).getByText(label, { exact: true }).waitFor();
        assert.equal((await current(p).textContent()).includes('尚无译文'), false); assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source);
        if (status === 'running') await current(p).getByText('正在返回的译文片段', { exact: true }).waitFor();
        if (status === 'failed' || status === 'running') await capture(p, 'translation-' + status);
      }
    } finally { await p.close(); }
  });
  await check('CP01A-action-failure-is-visible-and-retryable', async () => {
    const p = await app(); try {
      await p.evaluate(() => { const unit = window.uiHarness.state.current; window.uiHarness.readingProjectionRetranslations.push({ unitId: unit.unitId, textRevision: unit.textRevision, error: '受控服务拒绝重译' }); });
      await actions(p).getByRole('button', { name: '重新翻译本句', exact: true }).click(); await p.getByRole('alert').filter({ hasText: '受控服务拒绝重译' }).waitFor();
      assert.equal(await sourceText(current(p).getByTestId('source-text')), sample.source);
      await actions(p).getByRole('button', { name: '重新翻译本句', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.retranslations.length === 2);
      assert.equal(await p.getByRole('alert').filter({ hasText: '受控服务拒绝重译' }).count(), 0);
    } finally { await p.close(); }
  });
  await check('CP01A-all-eight-lexical-combinations-use-real-commands', async () => {
    const p = await app(); try {
      for (const segmentation of [true, false]) for (const ruby of [true, false]) for (const pos of [true, false]) {
        // Set POS while segmentation is enabled, exactly as the user can.
        const set = async (label, wanted) => { const button = options(p).getByRole('button', { name: label, exact: true }); if ((await button.getAttribute('aria-pressed')) !== String(wanted)) await button.click(); await p.waitForFunction(({ label, wanted }) => document.querySelector(`[aria-label="${label}"]`)?.getAttribute('aria-pressed') === String(wanted), { label, wanted }); };
        await set('可点击分词', true); await set('词性提示', pos); await set('假名注音', ruby); await set('可点击分词', segmentation);
        const source = current(p).getByTestId('source-text');
        assert.equal(await sourceText(source), sample.source); assert.equal(await source.getAttribute('data-segmentation'), String(segmentation)); assert.equal(await source.getAttribute('data-ruby'), String(ruby)); assert.equal(await source.getAttribute('data-pos'), String(segmentation && pos));
        assert.equal((await source.locator('button[data-token-index]').count()) > 0, segmentation); assert.equal((await source.locator('rt').count()) > 0, ruby);
        assert.equal(await options(p).getByRole('button', { name: '词性提示', exact: true }).isDisabled(), !segmentation);
      }
      const commands = await p.evaluate(() => window.uiHarness.commands.map(item => item.command.kind)); assert.ok(commands.includes('set_hud_preference')); assert.ok(commands.includes('set_reading_annotations'));
    } finally { await p.close(); }
  });
  await check('CP01A-new-sentence-keeps-frozen-word-card-preview-and-font-preferences', async () => {
    const p = await app(); try {
      await p.waitForFunction(() => window.uiHarness.previewAttaches.length === 1);
      await current(p).getByTestId('source-text').locator('button[data-token-index]').first().click(); await p.waitForFunction(() => window.uiHarness.wordCard !== null);
      const before = await p.evaluate(() => ({ word: structuredClone(window.uiHarness.wordCard), attach: window.uiHarness.previewAttaches.length, detach: window.uiHarness.previewDetaches.length, preferences: structuredClone(window.uiHarness.state.preferences) }));
      await p.evaluate(() => { const port = window.uiHarness, previous = port.state.current, unit = { ...structuredClone(previous), unitId: 'layout-next-unit', sourceText: '新的正文已经到达。', sourceLanguage: 'zh-CN', sourceHash: 'c'.repeat(64), tokens: [], translations: [], createdAtMs: previous.createdAtMs + 1000 }; port.publish({ commitRevision: port.state.commitRevision + 1, current: unit, history: { ...port.state.history, units: [unit, ...port.state.history.units] } }); });
      await current(p).getByTestId('source-text').getByText('新的正文已经到达。', { exact: true }).waitFor();
      assert.deepEqual(await p.evaluate(() => window.uiHarness.wordCard), before.word); assert.equal(await p.evaluate(() => window.uiHarness.wordCardRequests.length), 1);
      assert.equal(await p.evaluate(() => window.uiHarness.previewAttaches.length), before.attach); assert.equal(await p.evaluate(() => window.uiHarness.previewDetaches.length), before.detach);
      assert.equal(await current(p).getByTestId('source-text').locator('rt').count(), 0); assert.equal(await current(p).getByTestId('source-text').locator('button').count(), 0);
      assert.equal(await current(p).getByTestId('source-text').evaluate(node => getComputedStyle(node).fontSize), `${before.preferences.sourceFontPt}px`);
      assert.equal(await p.getByTestId('controlled-word-card-surface').count(), 1);
      await capture(p, 'new-sentence-frozen-word-card');
    } finally { await p.close(); }
  });
  for (const variant of [{ id: 'client1040', width: 1040, height: 900, zoom: 1 }, { id: 'client760', width: 760, height: 1100, zoom: 1 }, { id: 'css200', width: 1040, height: 900, zoom: 2 }, { id: 'wide-css200', width: 1160, height: 900, zoom: 2 }]) {
    await check('CP01A-layout-' + variant.id, async () => {
      const p = await app(variant); try {
        if (variant.zoom !== 1) await p.evaluate(zoom => { document.documentElement.style.zoom = String(zoom); }, variant.zoom);
        await current(p).scrollIntoViewIfNeeded();
        const measured = await p.evaluate(() => {
          const rect = node => { const r = node.getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom, width: r.width, height: r.height }; };
          const card = document.querySelector('[data-guided-current]'), action = document.querySelector('[aria-label="当前句操作"]'), options = document.querySelector('[aria-label="阅读控制"]');
          return { viewport: { width: innerWidth, height: innerHeight }, zoom: Number(getComputedStyle(document.documentElement).zoom), card: rect(card), action: rect(action), buttons: [...action.querySelectorAll('button')].map(rect), source: rect(card.querySelector('[data-testid="source-text"]')), options: rect(options), sourceFont: getComputedStyle(card.querySelector('[data-testid="source-text"]')).fontSize, translationFont: getComputedStyle(card.querySelector('article p')).fontSize, preferences: { source: window.uiHarness.state.preferences.sourceFontPt, translation: window.uiHarness.state.preferences.translationFontPt }, documentOverflow: document.documentElement.scrollWidth > innerWidth };
        });
        await fs.writeFile(path.join(out, variant.id + '.geometry.json'), JSON.stringify(measured, null, 2) + '\n');
        assert.equal(measured.documentOverflow, false); assert.ok(measured.card.width > 0 && measured.source.width > 0); assert.ok(measured.source.x >= measured.card.x && measured.source.right <= measured.card.right);
        for (const button of measured.buttons) { assert.ok(button.x >= measured.action.x && button.right <= measured.action.right, 'Action wraps within its own content region'); assert.ok(button.width > 0 && button.height > 0); }
        assert.equal(measured.sourceFont, `${measured.preferences.source}px`); assert.equal(measured.translationFont, `${measured.preferences.translation}px`);
        await capture(p, variant.id);
        const reachable = [];
        for (const group of [actions(p), options(p)]) for (const button of await group.getByRole('button').all()) {
          await button.scrollIntoViewIfNeeded();
          const target = await button.evaluate(node => { const r = node.getBoundingClientRect(); return { name: node.getAttribute('aria-label') || node.textContent, x: r.x, y: r.y, right: r.right, bottom: r.bottom, width: r.width, height: r.height, viewportWidth: innerWidth, viewportHeight: innerHeight, hit: node.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)) }; });
          assert.ok(target.x >= 0 && target.y >= 0 && target.right <= target.viewportWidth && target.bottom <= target.viewportHeight && target.width > 0 && target.height > 0, `Control can be reached after local scrolling: ${target.name}`); assert.equal(target.hit, true, `Control is unobscured: ${target.name}`); reachable.push(target);
        }
        await fs.writeFile(path.join(out, variant.id + '.actions.json'), JSON.stringify(reachable, null, 2) + '\n');
        await actions(p).scrollIntoViewIfNeeded(); await capture(p, variant.id + '-actions'); return measured;
      } finally { await p.close(); }
    });
  }
} catch (error) { errors.push({ message: error.message, stack: error.stack }); }
finally { if (browser) await browser.close(); if (server) await new Promise(resolve => server.close(resolve)); }
if (browserIdentity) { try { await verifyBrowserIdentity(browserIdentity); } catch (error) { errors.push({ message: error.message }); } }
const after = await hashes(); if (JSON.stringify(before) !== JSON.stringify(after)) errors.push({ message: 'Owned implementation or runner changed during execution' });
if (pageErrors.length || consoleErrors.length || externalRequests.length) errors.push({ message: 'Browser error or unexpected external request', pageErrors, consoleErrors, externalRequests });
const result = { schemaVersion: 1, id: 'cp01a-current-reading-composition', browserIdentity, nonce: randomUUID(), startedAt, finishedAt: new Date().toISOString(), status: errors.length === 0 && checks.length === 12 && checks.every(item => item.status === 'passed') ? 'passed' : 'failed', checks, captures, builds, sourceSha256: before, sourceSha256After: after, errors, layer: 'production_react_controlled_transport', nativeOrGameVerified: false, pixelAcceptance: false, productReleaseApproved: false, limitations: ['No independent reference-pixel approval', 'No native capture, actual database, real translation service or game acceptance', 'Existing preference fonts preserved; independent design typography contract remains separately reviewed'] };
await fs.writeFile(path.join(out, 'result.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ output: out, status: result.status, checks: checks.length, failures: checks.filter(item => item.status !== 'passed').map(item => item.id), errors }, null, 2));
process.exitCode = result.status === 'passed' ? 0 : 1;
