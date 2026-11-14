/** CP-11 production React journeys; controlled transport is not Native/SQLite proof. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash, randomUUID } from 'node:crypto';
import { launchLockedBrowser, verifyBrowserIdentity } from './locked-browser.mjs';
import nodeAssert from 'node:assert/strict';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..'), desktop = path.join(root, 'apps/desktop');
const argument = name => process.argv[process.argv.indexOf(name) + 1];
const out = path.resolve(root, process.argv.includes('--output') ? argument('--output') : `.build/current-client-execution-20261105/history/run-${Date.now()}`);
if (!out.startsWith(path.join(root, '.build') + path.sep) || fsSync.existsSync(out)) throw Error('A fresh repository .build directory is required');
await fs.mkdir(out, { recursive: true });
const nonce = randomUUID(), sha = bytes => createHash('sha256').update(bytes).digest('hex'), checks = [], screenshots = [], browserErrors = [], executions = [];
let assertionCount = 0, browser, browserIdentity, server;
const assert = new Proxy(nodeAssert, { apply(fn, self, args) { assertionCount++; return Reflect.apply(fn, self, args); }, get(fn, key) { const value = fn[key]; return typeof value === 'function' ? (...args) => { assertionCount++; return Reflect.apply(value, fn, args); } : value; } });
const owned = ['apps/desktop/src/features/history/HistoryScreen.tsx', 'apps/desktop/src/features/history/styles.module.css', 'apps/desktop/src/features/history/useHistoryQueries.ts', 'apps/desktop/tests/support/history-fixture-port.ts', 'apps/desktop/tests/visual/verify-history.mjs', 'apps/desktop/tests/visual/locked-browser.mjs'];
const sourceIdentity = async () => Object.fromEntries(await Promise.all(owned.map(async file => [file, sha(await fs.readFile(path.join(root, file)))])));
const before = await sourceIdentity(), startedAt = new Date().toISOString();
const free = createServer(); await new Promise(resolve => free.listen(0, '127.0.0.1', resolve)); const port = free.address().port; await new Promise(resolve => free.close(resolve));
const origin = `http://127.0.0.1:${port}`;
async function command(args, label) {
  const child = spawn(process.execPath, args, { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] }); let stdout = '', stderr = '';
  child.stdout.on('data', value => stdout += value); child.stderr.on('data', value => stderr += value);
  const exitCode = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); });
  executions.push({ command: [process.execPath, ...args], exitCode }); await fs.writeFile(path.join(out, `${label}.json`), JSON.stringify({ ...executions.at(-1), stdout, stderr }, null, 2));
  assert.equal(exitCode, 0, `${label}: ${stderr || stdout}`);
}
async function check(id, fn) {
  const count = assertionCount;
  try { const facts = await fn(); checks.push({ id, status: 'passed', assertions: assertionCount - count, facts }); console.log(`${id}: passed`); }
  catch (error) { checks.push({ id, status: 'failed', assertions: assertionCount - count, error: error.message, stack: error.stack }); console.log(`${id}: failed: ${error.stack}`); }
}
async function select(page, name, value) {
  await page.getByRole('combobox', { name, exact: true }).click();
  const options = page.getByRole('option');
  const index = await options.evaluateAll((nodes, expected) => nodes.findIndex(node => node.getAttribute('data-select-value') === expected), value);
  assert.ok(index >= 0, `Visible Select option is missing: ${value}`);
  await options.nth(index).click();
}
async function capture(page, name) {
  await page.evaluate(() => document.fonts.ready);
  const file = `${name}.png`; await page.screenshot({ path: path.join(out, file), animations: 'disabled' }); screenshots.push({ file, sha256: sha(await fs.readFile(path.join(out, file))), viewport: page.viewportSize(), scope: 'whole_browser_client', native: false, approvedReference: false });
}
async function open({ viewport = { width: 1040, height: 720 }, navigate = true } = {}) {
  const page = await browser.newPage({ viewport, deviceScaleFactor: 1 }); page.setDefaultTimeout(9000);
  page.on('pageerror', error => browserErrors.push(error.message));
  await page.goto(`${origin}/tests/visual/index.html?role=main&scenario=reading`);
  await page.waitForFunction(() => !!window.uiHarness && !!window.uiHistoryHarness);
  await page.evaluate(() => Object.assign(document.getElementById('root').style, { inset: '0', width: '100%', height: '100%' }));
  if (navigate) { await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.locator('[aria-label="历史筛选与记录"] button[aria-pressed]').first().waitFor(); }
  return page;
}
const rows = page => page.locator('[aria-label="历史筛选与记录"] button[aria-pressed]');
const detailText = page => page.locator('[aria-label="记录详情"] [data-testid="source-text"]');
try {
  await command([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck');
  const config = path.join(out, 'vite-history.config.mjs'), bundle = path.join(out, 'controlled-build');
  const originalBootstrap = await fs.readFile(path.join(desktop, 'tests/support/bootstrap.tsx'), 'utf8');
  const insertion = 'Object.assign(window, { uiHarness: port });'; assert.equal(originalBootstrap.split(insertion).length, 2, 'The existing controlled transport bootstrap must have one exact installation seam');
  const source = `import react from ${JSON.stringify(path.join(desktop, 'node_modules/@vitejs/plugin-react/dist/index.js'))};\nexport default { root: ${JSON.stringify(desktop)}, plugins: [react(), { name: 'cp11-controlled-query-responses', transform(code,id) { if (!id.endsWith('/tests/support/bootstrap.tsx')) return; return { code: "import { installHistoryFixture } from './history-fixture-port';\\n" + code.replace(${JSON.stringify(insertion)}, ${JSON.stringify(insertion + '\nObject.assign(window, { uiHistoryHarness: installHistoryFixture(port) });')}), map:null }; } }], build: { target:'es2022', outDir:${JSON.stringify(bundle)}, emptyOutDir:true, rollupOptions:{input:${JSON.stringify(path.join(desktop, 'tests/visual/index.html'))}} } };\n`;
  await fs.writeFile(config, source);
  await command([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', config], 'controlled-build');
  server = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--outDir', bundle, '--host', '127.0.0.1', '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let serverLog = ''; server.stdout.on('data', data => serverLog += data); server.stderr.on('data', data => serverLog += data);
  for (let attempt = 0; attempt < 120; attempt++) { try { if ((await fetch(`${origin}/tests/visual/index.html`)).ok) break; } catch {} if (attempt === 119 || server.exitCode !== null) throw Error(`History test host failed: ${serverLog}`); await new Promise(resolve => setTimeout(resolve, 100)); }
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());
  await check('CP11-01-r79-areas-and-independent-pagination', async () => {
    const page = await open(); assert.equal(await rows(page).count(), 50);
    const recordReads = await page.evaluate(() => window.uiHistoryHarness.recordQueries.length);
    await page.getByRole('button', { name: '加载更早会话', exact: true }).click();
    await page.getByText('没有更早会话', { exact: true }).waitFor();
    assert.equal(await page.evaluate(() => window.uiHistoryHarness.recordQueries.length), recordReads);
    const sessionReads = await page.evaluate(() => window.uiHistoryHarness.sessionQueries.length);
    await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 100);
    assert.equal(await page.evaluate(() => window.uiHistoryHarness.sessionQueries.length), sessionReads);
    const geometry = await page.evaluate(() => { const filter = document.querySelector('[aria-label="游戏筛选"]'); const list = document.querySelector('[aria-label="历史筛选与记录"]'); const detail = document.querySelector('[aria-label="记录详情"]'); return { filter: filter.getBoundingClientRect().toJSON(), list: list.getBoundingClientRect().toJSON(), detail: detail.getBoundingClientRect().toJSON(), pageOverflow: document.documentElement.scrollWidth > innerWidth }; });
    assert.ok(geometry.filter.bottom <= geometry.list.top); assert.ok(geometry.list.right <= geometry.detail.left + 1); assert.equal(geometry.pageOverflow, false);
    await capture(page, 'normal-1040x720'); await page.close(); return geometry;
  });
  await check('CP11-02-append-retry-retains-cursor-selection-and-scroll', async () => {
    const page = await open(); await rows(page).nth(12).click(); await detailText(page).filter({ hasText: '甲游戏记录 12' }).waitFor();
    await page.evaluate(() => { window.uiHistoryHarness.faults.push({ kind: 'records', error: 'controlled append failure' }); document.querySelector('[aria-label="可滚动的历史记录"]').scrollTop = 250; });
    await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.getByRole('alert').filter({ hasText: 'controlled append failure' }).waitFor();
    assert.equal(await rows(page).count(), 50); assert.ok((await detailText(page).innerText()).includes('甲游戏记录 12'));
    assert.ok(await page.locator('[aria-label="可滚动的历史记录"]').evaluate(node => node.scrollTop > 0));
    await page.getByRole('button', { name: '重新加载记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 100);
    const cursors = await page.evaluate(() => window.uiHistoryHarness.recordQueries.slice(-2).map(query => query.cursor)); assert.equal(cursors[0], cursors[1]); assert.notEqual(cursors[0], null);
    assert.ok((await detailText(page).innerText()).includes('甲游戏记录 12')); await page.close();
  });
  await check('CP11-03-replace-failure-keeps-readable-old-scope-and-exact-retry', async () => {
    const page = await open(); await rows(page).nth(4).click();
    await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'records', gameId: 'history-game-b', error: 'controlled replace failure' }));
    await select(page, '游戏筛选', 'history-game-b'); await page.getByRole('alert').filter({ hasText: '新范围加载失败，仍保留上一范围的记录' }).waitFor();
    assert.equal(await rows(page).count(), 50); assert.ok((await rows(page).first().innerText()).includes('甲游戏记录 0')); assert.ok((await detailText(page).innerText()).includes('甲游戏记录 4'));
    assert.equal(await page.getByText('上一范围的记录', { exact: true }).count(), 1);
    await capture(page, 'replace-failed-old-scope-retained');
    await page.getByRole('button', { name: '重新加载记录', exact: true }).click(); await rows(page).first().filter({ hasText: '乙游戏记录 65' }).waitFor();
    const queries = await page.evaluate(() => window.uiHistoryHarness.recordQueries.slice(-2)); assert.deepEqual(queries.map(query => query.cursor), [null, null]); assert.ok(queries.every(query => query.filter.gameId === 'history-game-b'));
    await page.close();
  });
  await check('CP11-04-superseded-completion-cannot-clear-current-pending', async () => {
    const page = await open(); await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'records', gameId: 'history-game-a', hold: 'late-a' }, { kind: 'sessions', gameId: 'history-game-a', hold: 'late-a-sessions' }, { kind: 'records', gameId: 'history-game-b', hold: 'current-b' }));
    await select(page, '游戏筛选', 'history-game-a'); await page.waitForFunction(() => window.uiHistoryHarness.pending.has('late-a') && window.uiHistoryHarness.pending.has('late-a-sessions'));
    await select(page, '游戏筛选', 'history-game-b'); await page.waitForFunction(() => window.uiHistoryHarness.pending.has('current-b'));
    await page.evaluate(() => { window.uiHistoryHarness.release('late-a'); window.uiHistoryHarness.release('late-a-sessions'); });
    await page.waitForTimeout(100); assert.equal(await page.locator('[aria-label="可滚动的历史记录"]').getAttribute('aria-busy'), 'true');
    await page.evaluate(() => window.uiHistoryHarness.release('current-b')); await rows(page).first().filter({ hasText: '乙游戏记录 65' }).waitFor();
    assert.equal(await page.locator('[aria-label="可滚动的历史记录"]').getAttribute('aria-busy'), 'false'); await page.close();
  });
  await check('CP11-05-catalog-append-failure-retries-only-directory', async () => {
    const page = await open(); const count = await page.evaluate(() => window.uiHistoryHarness.recordQueries.length);
    await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'sessions', error: 'controlled catalogue append failure' }));
    await page.getByRole('button', { name: '加载更早会话', exact: true }).click(); await page.getByText('加载失败 · 已有会话仍可选', { exact: true }).waitFor();
    await page.getByRole('button', { name: '重新加载会话', exact: true }).click(); await page.getByText('没有更早会话', { exact: true }).waitFor();
    assert.equal(await page.evaluate(() => window.uiHistoryHarness.recordQueries.length), count);
    const cursors = await page.evaluate(() => window.uiHistoryHarness.sessionQueries.slice(-2).map(query => query.cursor)); assert.equal(cursors[0], cursors[1]); assert.notEqual(cursors[0], null); await page.close();
  });
  await check('CP11-06-edit-freezes-navigation-caret-and-multipage-window', async () => {
    const page = await open(); await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 100);
    await rows(page).nth(61).click(); await detailText(page).filter({ hasText: '甲游戏记录 61' }).waitFor();
    await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'sessions', error: 'directory can retry during edit' }));
    await page.getByRole('button', { name: '加载更早会话', exact: true }).click(); await page.getByRole('button', { name: '重新加载会话', exact: true }).waitFor();
    const counts = await page.evaluate(() => [window.uiHistoryHarness.recordQueries.length, window.uiHistoryHarness.sessionQueries.length]);
    await page.getByRole('button', { name: '修正 历史测试引擎', exact: true }).click();
    const input = page.getByRole('textbox', { name: '人工修正', exact: true }); await input.fill('保存失败后仍然保留这段草稿。');
    await input.evaluate(node => { node.setSelectionRange(3, 8); node.focus(); });
    await page.evaluate(() => window.uiHarness.publish({ commitRevision: window.uiHarness.state.commitRevision + 1, current: { ...window.uiHarness.state.current, sourceText: '游戏继续推进的新句。' } }));
    assert.equal(await input.inputValue(), '保存失败后仍然保留这段草稿。'); assert.deepEqual(await input.evaluate(node => [node.selectionStart, node.selectionEnd, node === document.activeElement]), [3, 8, true]);
    for (const name of ['游戏筛选', '会话筛选']) assert.equal(await page.getByRole('combobox', { name, exact: true }).isDisabled(), true);
    for (const name of ['刷新记录', '定位当前句', '加载更早记录', '重新加载会话']) assert.equal(await page.getByRole('button', { name, exact: true }).isDisabled(), true);
    assert.equal(await rows(page).first().isDisabled(), true);
    await page.evaluate(() => { window.uiHarness.failNextCommand = true; }); await page.getByRole('button', { name: '保存修正', exact: true }).click(); await page.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor();
    assert.equal(await input.inputValue(), '保存失败后仍然保留这段草稿。'); assert.deepEqual(await input.evaluate(node => [node.selectionStart, node.selectionEnd, node === document.activeElement]), [3, 8, true]);
    await capture(page, 'editing-failed-caret-preserved');
    await page.getByRole('button', { name: '取消编辑', exact: true }).click(); await input.waitFor({ state: 'hidden' });
    assert.equal(await rows(page).count(), 100); assert.deepEqual(await page.evaluate(() => [window.uiHistoryHarness.recordQueries.length, window.uiHistoryHarness.sessionQueries.length]), counts);
    assert.ok((await detailText(page).innerText()).includes('甲游戏记录 61')); await page.close();
  });
  await check('CP11-07-locate-current-traverses-all-pages-without-duplicates', async () => {
    const page = await open(); await page.getByRole('button', { name: '定位当前句', exact: true }).click(); await detailText(page).filter({ hasText: '乙游戏记录 120' }).waitFor();
    assert.equal(await rows(page).count(), 51);
    await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 101);
    await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.getByText('已到最早记录', { exact: true }).waitFor();
    const sources = await rows(page).locator('> span').allTextContents(); assert.equal(sources.length, 139); assert.equal(new Set(sources).size, 139); assert.ok(sources.includes('甲游戏记录 50')); assert.ok(sources.includes('手动输入记录 138')); assert.equal(sources.filter(value => value === '乙游戏记录 120').length, 1); assert.ok((await detailText(page).innerText()).includes('乙游戏记录 120'));
    await page.close();
  });
  await check('CP11-08-new-projection-does-not-refresh-pages-or-reset-scroll', async () => {
    const page = await open(); await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 100);
    await rows(page).nth(70).click(); await detailText(page).filter({ hasText: '乙游戏记录 70' }).waitFor();
    const count = await page.evaluate(() => { document.querySelector('[aria-label="可滚动的历史记录"]').scrollTop = 600; return window.uiHistoryHarness.recordQueries.length; });
    await page.evaluate(() => { const port = window.uiHarness; const newer = { ...structuredClone(port.state.history.units[0]), unitId: 'background-new-unit', sourceText: '后台最新一句', createdAtMs: port.state.history.units[0].createdAtMs + 1000 }; port.publish({ current: newer, history: { units: [newer, ...port.state.history.units], nextCursor: null }, commitRevision: port.state.commitRevision + 1 }); });
    await page.waitForTimeout(100); assert.equal(await rows(page).count(), 100); assert.equal(await page.evaluate(() => window.uiHistoryHarness.recordQueries.length), count); assert.ok((await detailText(page).innerText()).includes('乙游戏记录 70')); assert.ok(await page.locator('[aria-label="可滚动的历史记录"]').evaluate(node => node.scrollTop > 500));
    await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.getByText('已到最早记录', { exact: true }).waitFor(); assert.equal(await rows(page).count(), 139); assert.equal(await rows(page).filter({ hasText: '后台最新一句' }).count(), 0); await page.close();
  });
  await check('CP11-09-detail-retry-does-not-refresh-list', async () => {
    const page = await open(); await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'detail', error: 'controlled detail failure' })); await rows(page).nth(7).click(); await page.getByRole('alert').filter({ hasText: 'controlled detail failure' }).waitFor();
    assert.ok((await detailText(page).innerText()).includes('甲游戏记录 7')); const reads = await page.evaluate(() => window.uiHistoryHarness.recordQueries.length);
    await page.getByRole('button', { name: '重新读取详情', exact: true }).click(); await page.getByRole('alert').filter({ hasText: 'controlled detail failure' }).waitFor({ state: 'hidden' }); assert.equal(await page.evaluate(() => window.uiHistoryHarness.recordQueries.length), reads); await page.close();
  });
  await check('CP11-10-narrow-and-css200-readability', async () => {
    const facts = [];
    for (const [name, viewport, zoom] of [['narrow', { width: 740, height: 1200 }, 1], ['css200', { width: 1040, height: 1200 }, 2]]) {
      const page = await open({ viewport }); await page.evaluate(value => document.documentElement.style.zoom = String(value), zoom); await page.evaluate(() => document.fonts.ready);
      const result = await page.evaluate(() => { const nodes = [...document.querySelectorAll('[aria-label="历史记录工作区"] button, [aria-label="历史记录工作区"] [role="combobox"]')].filter(node => node.getBoundingClientRect().width && !node.closest('[aria-label="可滚动的历史记录"]')); return { documentWidth: document.documentElement.scrollWidth, viewport: innerWidth, controlsOutside: nodes.filter(node => { const rect = node.getBoundingClientRect(); return rect.left < -1 || rect.right > innerWidth + 1; }).map(node => node.textContent) }; });
      assert.equal(result.controlsOutside.length, 0); assert.ok(result.documentWidth <= result.viewport + 1); await capture(page, `layout-${name}`); facts.push({ name, ...result }); await page.close();
    }
    return facts;
  });
  await check('CP11-11-owner-generation-rejects-old-completion', async () => {
    const page = await open(); await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'records', gameId: 'history-game-a', hold: 'old-owner' })); await select(page, '游戏筛选', 'history-game-a'); await page.waitForFunction(() => window.uiHistoryHarness.pending.has('old-owner'));
    await page.evaluate(() => { const port = window.uiHarness; const unit = { ...structuredClone(port.state.history.units[0]), unitId: 'new-owner-unit', sourceText: '新配置档的记录' }; port.publish({ profileGeneration: port.state.profileGeneration + 1, history: { units: [unit], nextCursor: null }, current: unit }); });
    await rows(page).first().filter({ hasText: '新配置档的记录' }).waitFor(); await page.evaluate(() => window.uiHistoryHarness.release('old-owner')); await page.waitForTimeout(100); assert.equal(await rows(page).count(), 1); assert.ok((await rows(page).first().innerText()).includes('新配置档的记录')); await page.close();
  });
  await check('CP11-12-long-content-scrolls-in-panes-without-filter-overflow', async () => {
    const page = await open({ viewport: { width: 740, height: 1200 }, navigate: false });
    await page.evaluate(() => {
      const port = window.uiHarness;
      const units = structuredClone(port.state.history.units), games = structuredClone(port.state.games);
      games[0].displayName = '名称非常长的游戏 · '.repeat(18);
      units[0].sourceText = '放課後、図書館で彼女と待ち合わせをした。\n'.repeat(24);
      units[0].translations[0].text = '放学后，我和她约好在图书馆见面。\n'.repeat(30);
      port.publish({ history: { units, nextCursor: null }, games });
    });
    await page.getByRole('button', { name: '历史记录', exact: true }).click(); await rows(page).first().waitFor();
    const geometry = await page.evaluate(() => {
      const detail = document.querySelector('[aria-label="记录详情"]'), list = document.querySelector('[aria-label="可滚动的历史记录"]'), filter = document.querySelector('[aria-label="游戏筛选"]');
      const before = filter.getBoundingClientRect().toJSON(); detail.scrollTop = 400;
      return { detailScrollable: detail.scrollHeight > detail.clientHeight, detailScroll: detail.scrollTop, listScroll: list.scrollTop, filterBefore: before, filterAfter: filter.getBoundingClientRect().toJSON(), documentWidth: document.documentElement.scrollWidth, width: innerWidth };
    });
    assert.equal(geometry.detailScrollable, true); assert.ok(geometry.detailScroll >= 400); assert.equal(geometry.listScroll, 0); assert.deepEqual(geometry.filterAfter, geometry.filterBefore); assert.ok(geometry.documentWidth <= geometry.width + 1);
    await capture(page, 'long-content-narrow-scroll'); await page.close(); return geometry;
  });
  await check('CP11-13-correction-success-and-hidden-page-preserve-multiple-pages', async () => {
    const page = await open(); await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 100);
    await rows(page).nth(56).click(); await detailText(page).filter({ hasText: '甲游戏记录 56' }).waitFor();
    await page.getByRole('button', { name: '修正 历史测试引擎', exact: true }).click(); const input = page.getByRole('textbox', { name: '人工修正', exact: true }); await input.fill('这份修正固定在第五十六条记录。');
    const recordReads = await page.evaluate(() => window.uiHistoryHarness.recordQueries.length);
    await page.getByRole('button', { name: '手动翻译', exact: true }).click(); await page.getByRole('button', { name: '历史记录', exact: true }).click(); assert.equal(await input.inputValue(), '这份修正固定在第五十六条记录。');
    await page.evaluate(() => window.uiHistoryHarness.correctionResponses.push({ translationId: 'history-translation-56', baseRevision: 0, correction: { revision: 1, text: '这份修正固定在第五十六条记录。' } }));
    await page.getByRole('button', { name: '保存修正', exact: true }).click(); await input.waitFor({ state: 'hidden' }); await page.locator('[aria-label="记录详情"] p').filter({ hasText: /^这份修正固定在第五十六条记录。$/ }).waitFor();
    assert.equal(await rows(page).count(), 100); assert.equal(await page.evaluate(() => window.uiHistoryHarness.recordQueries.length), recordReads);
    const command = await page.evaluate(() => window.uiHarness.commands.at(-1).command); assert.equal(command.kind, 'save_correction'); assert.equal(command.payload.translationId, 'history-translation-56'); assert.equal(command.payload.baseRevision, 0);
    const other = await page.evaluate(() => window.uiHarness.state.history.units[0].translations[0].correction); assert.equal(other, null); await page.close();
  });
  await check('CP11-14-history-word-card-retains-frozen-object-on-game-advance', async () => {
    const page = await open({ navigate: false });
    await page.evaluate(() => {
      const port = window.uiHarness, units = structuredClone(port.state.history.units);
      units[0].sourceText = '言葉'; units[0].tokens = [{ surface: '言葉', lemma: '言葉', reading: 'ことば', partOfSpeech: '名詞', byteStart: 0, byteEnd: 6, utf16Start: 0, utf16End: 2, known: true }]; port.publish({ history: { units, nextCursor: null } });
    });
    await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.locator('[aria-label="记录详情"]').getByRole('button', { name: '言葉，读音 ことば', exact: true }).click(); await page.getByTestId('controlled-word-card-surface').waitFor();
    const frozen = await page.evaluate(() => window.uiHarness.wordCardSnapshot());
    await page.evaluate(() => { const port = window.uiHarness; port.publish({ current: { ...structuredClone(port.state.current), sourceText: '游戏已经推进到下一句。' }, commitRevision: port.state.commitRevision + 1 }); });
    await page.waitForTimeout(100); assert.equal(await page.getByTestId('controlled-word-card-surface').count(), 1); assert.deepEqual(await page.evaluate(() => window.uiHarness.wordCardSnapshot()), frozen); assert.equal(await page.evaluate(() => window.uiHarness.wordCardRequests.length), 1); await capture(page, 'fixed-history-word-card'); await page.close();
  });
  await check('CP11-15-catalog-pending-resumes-after-hidden-editor-unblocks', async () => {
    const page = await open();
    await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'sessions', hold: 'interrupted-catalog-append' }));
    await page.getByRole('button', { name: '加载更早会话', exact: true }).click();
    await page.waitForFunction(() => window.uiHistoryHarness.pending.has('interrupted-catalog-append'));
    const before = await page.evaluate(() => window.uiHistoryHarness.sessionQueries.length);
    await page.getByRole('button', { name: '修正 历史测试引擎', exact: true }).click();
    const input = page.getByRole('textbox', { name: '人工修正', exact: true });
    await input.fill('编辑期间目录加载不能永久挂起。');
    await page.getByRole('button', { name: '手动翻译', exact: true }).click();
    await page.getByRole('button', { name: '历史记录', exact: true }).click();
    assert.equal(await input.inputValue(), '编辑期间目录加载不能永久挂起。');
    await page.getByRole('button', { name: '取消编辑', exact: true }).click();
    await input.waitFor({ state: 'hidden' });
    await page.waitForTimeout(200);
    await page.evaluate(() => window.uiHistoryHarness.release('interrupted-catalog-append'));
    await page.waitForTimeout(250);
    const facts = await page.evaluate(before => ({ before, queries: window.uiHistoryHarness.sessionQueries, directoryText: [...document.querySelectorAll('small[role=status]')].map(node => node.textContent) }), before);
    const pendingButton = page.getByRole('button', { name: '加载更早会话', exact: true });
    facts.buttonStillDisabled = await pendingButton.count() ? await pendingButton.isDisabled() : false;
    await fs.writeFile(path.join(out, 'adversarial-observations.json'), JSON.stringify(facts, null, 2));
    await capture(page, 'interrupted-catalog-after-cancel');
    assert.equal(facts.queries.length, before + 1, 'Cancelling edit must resume the interrupted directory append using its original cursor');
    await page.getByText('没有更早会话', { exact: true }).waitFor();
    await page.close();return facts;
  });
  await check('CP11-16-short-client-at-200-percent-can-reach-detail-and-edit', async () => {
    const facts = [];
    for (const [name, viewport, zoom] of [['short-narrow', { width: 740, height: 560 }, 1], ['short-css200', { width: 1040, height: 720 }, 2]]) {
      const page = await open({ viewport });
      await page.evaluate(value => document.documentElement.style.zoom = String(value), zoom);
      await page.evaluate(() => document.fonts.ready);
      const edit = page.getByRole('button', { name: '修正 历史测试引擎', exact: true });
      await edit.click();
      const input = page.getByRole('textbox', { name: '人工修正', exact: true });
      await input.fill('放大和短窗口中仍能查看并编辑这条译文。');
      const cancel = page.getByRole('button', { name: '取消编辑', exact: true });
      await cancel.scrollIntoViewIfNeeded();
      const result = await cancel.evaluate(node => {
        const workspace = document.querySelector('[aria-label="历史记录工作区"]');
        const detail = document.querySelector('[aria-label="记录详情"]');
        return { action: node.getBoundingClientRect().toJSON(), detail: detail.getBoundingClientRect().toJSON(), workspace: workspace.getBoundingClientRect().toJSON(), workspaceScrollable: workspace.scrollHeight > workspace.clientHeight, workspaceScroll: workspace.scrollTop, width: innerWidth, height: innerHeight, documentWidth: document.documentElement.scrollWidth };
      });
      assert.ok(result.action.top >= result.workspace.top - 1 && result.action.bottom <= Math.min(result.height, result.workspace.bottom) + 1, 'Detail actions must be reachable within the actual client height');
      assert.ok(result.action.left >= 0 && result.action.right <= result.width + 1);
      assert.ok(result.detail.top < Math.min(result.height, result.workspace.bottom) && result.detail.bottom > result.workspace.top);
      assert.ok(result.documentWidth <= result.width + 1);
      await capture(page, `reachable-${name}`);
      await cancel.click(); await input.waitFor({ state: 'hidden' });
      assert.equal(await rows(page).count(), 50);
      facts.push({ name, zoom, ...result }); await page.close();
    }
    return facts;
  });
  assert.equal(browserErrors.length, 0, browserErrors.join('\n'));
} catch (error) { checks.push({ id: 'CP11-runner', status: 'failed', assertions: assertionCount, error: error.message, stack: error.stack }); console.error(error.stack); }
finally {
  await browser?.close(); server?.kill('SIGTERM');
  if (browserIdentity) { try { await verifyBrowserIdentity(browserIdentity); } catch (error) { browserErrors.push(error.message); } }
  const after = await sourceIdentity(), stableOwnedSources = JSON.stringify(before) === JSON.stringify(after);
  if (!stableOwnedSources) checks.push({ id: 'CP11-owned-source-epoch', status: 'failed', assertions: 1, error: 'History implementation changed during the run' });
  const result = { schema_version: 1, kind: 'history-production-react-journeys/v1', status: checks.length === 16 && checks.every(check => check.status === 'passed') && !browserErrors.length && stableOwnedSources ? 'passed' : 'failed', nonce, browserIdentity, startedAt, finishedAt: new Date().toISOString(), checks, assertionCount, browserErrors, screenshots, executions, ownedSourceHashes: after, stableOwnedSources, scope: 'production HistoryScreen with controlled query responses', nativeAcceptance: false, repositoryAcceptance: false, referenceApproval: false, productCompletion: 'incomplete' };
  await fs.writeFile(path.join(out, 'verification.json'), JSON.stringify(result, null, 2) + '\n'); console.log(`History result: ${result.status}; ${path.join(out, 'verification.json')}`); if (result.status !== 'passed') process.exitCode = 1;
}
