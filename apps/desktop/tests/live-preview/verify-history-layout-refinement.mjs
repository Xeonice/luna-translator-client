/** Approved history layout regression on production WindowRoot and existing query ports.
 * Controlled transport, browser geometry and screenshots do not prove Native/SQLite. */
import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createServer as createHttpServer } from 'node:http';
import { launchLockedBrowser, verifyBrowserIdentity } from '../visual/locked-browser.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const evidenceRoot = path.join(root, '.build/current-client-execution-20261105/layout-refinement-implementation-20261107');
const output = path.join(evidenceRoot, 'history', 'run-' + Date.now());
const workspace = path.join(output, 'workspace'), desktop = path.join(workspace, 'apps/desktop');
const screen = 'apps/desktop/src/features/history/HistoryScreen.tsx', css = 'apps/desktop/src/features/history/styles.module.css';
const designRoot = path.join(root, 'docs/design/first-product-v1');
const designEntry = 'candidates/layout-refinements-20261107/history.html';
const tracked = [screen, css, 'apps/desktop/src/ui/primitives/Select.tsx', 'apps/desktop/src/ui/primitives/controls.module.css', 'apps/desktop/src/features/history/useHistoryQueries.ts', 'apps/desktop/tests/support/history-fixture-port.ts', 'apps/desktop/tests/live-preview/verify-history-layout-refinement.mjs', 'docs/design/first-product-v1/' + designEntry, 'docs/design/first-product-v1/candidates/layout-refinements-20261107/history-settings.css'];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let browser, identity, server, staticServer, origin, designOrigin, sourceBefore, fixedBefore, beforeSource, candidateSource, serverLog = '';
const cases = [], pages = new Set();
const check = (name, run) => test(name, { timeout: 60000 }, async () => {
  try { const value = await run(); cases.push({ name, status: 'passed', value }); }
  catch (error) { cases.push({ name, status: 'failed', error: error.stack }); throw error; }
});
const record = (name, value) => fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n');
async function sourceHashes() { return Object.fromEntries(await Promise.all(tracked.map(async name => [name, hash(await fs.readFile(path.join(root, name)))]))); }
async function fixedHealth() { try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(500) })).json(); } catch { return null; } }
async function frame(page) { await page.evaluate(() => document.fonts.ready); await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
const rows = page => page.locator('[aria-label="历史筛选与记录"] button[aria-pressed]');
const detailText = page => page.locator('[aria-label="记录详情"] [data-testid="source-text"]');
async function open({ width = 1040, height = 720, zoom = 1, theme = 'dark', navigate = true } = {}) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 }); pages.add(page); page.setDefaultTimeout(7000);
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${origin}/client.html?role=main&view=${navigate ? 'history' : 'manual'}&state=normal&theme=${theme}`);
  await page.waitForFunction(() => Boolean(window.uiHistoryHarness && window.liveUiPreview?.port));
  if (navigate) { await rows(page).first().waitFor(); await page.locator('[aria-label="记录详情"][aria-busy="false"]').waitFor(); }
  if (zoom !== 1) await page.evaluate(value => document.documentElement.style.zoom = String(value), zoom);
  await frame(page); return { page, errors };
}
async function capture(context, name) { await frame(context.page); await context.page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' }); assert.deepEqual(context.errors, []); }
async function finish(context, name) { await capture(context, name); await context.page.close(); pages.delete(context.page); }
async function select(page, name, value) {
  await page.getByRole('combobox', { name, exact: true }).click();
  const options = page.getByRole('option'), index = await options.evaluateAll((nodes, value) => nodes.findIndex(node => node.getAttribute('data-select-value') === value), value);
  assert.ok(index >= 0, 'Shared Select must expose requested option ' + value); await options.nth(index).click();
}
async function geometry(page, design = false) {
  return page.evaluate(design => {
    const find = (actual, reference) => document.querySelector(design ? reference : actual);
    const scale = Number(getComputedStyle(document.documentElement).zoom) || 1;
    const box = node => { if (!node) return null; const r = node.getBoundingClientRect(); return Object.fromEntries(['x', 'y', 'width', 'height', 'left', 'right', 'top', 'bottom'].map(key => [key, r[key] / scale])); };
    const pane = find('[aria-label="历史筛选与记录"]', '.hx-list-column'), detail = find('[aria-label="记录详情"]', '.hx-detail');
    const game = find('[role=combobox][aria-label="游戏筛选"]', '#hx-game-filter--trigger'), session = find('[role=combobox][aria-label="会话筛选"]', '#hx-session-filter--trigger');
    const filters = game.closest(design ? '.hx-filters' : 'section > div'), tools = session.closest(design ? '.layout-history-session-tools' : 'section > div > div');
    const action = design ? document.querySelector('#hx-sessions-more') : tools.querySelector('button:not([role=combobox]), span[class*="directoryEnd"]');
    const label = design ? tools.querySelector('.hx-filter > span:first-child') : tools.querySelector('[data-field-label]');
    const status = design ? document.querySelector('#hx-session-status') : tools.querySelector('small[role=status]');
    const detailCss = getComputedStyle(detail), record = find('[aria-label="历史筛选与记录"] button[aria-pressed] > span', '.hx-record-source');
    return { scale, game: box(game), session: box(session), gameLabel: box(design ? game.closest('.hx-filter').querySelector('span:first-child') : game.parentElement.querySelector('[data-field-label]')), label: box(label), action: box(action), status: box(status), filters: box(filters), list: box(pane), detail: box(detail), heading: box(find('[aria-label="历史筛选与记录"] header strong', '.hx-list-heading strong')), recordText: box(record), meta: box(find('[aria-label="记录详情"] > div[class*="meta"]', '.hx-detail-topline')), original: box(find('[aria-label="记录详情"] > div[class*="original"]', '.hx-source')), detailPadding: [detailCss.paddingLeft, detailCss.paddingRight], navigation: box(design ? document.querySelector('.main-sidebar') : document.querySelector('nav').closest('aside')), documentOverflow: document.documentElement.scrollWidth > innerWidth, trackRatio: box(pane).width / box(detail).width, stacked: box(detail).top > box(pane).top + 1 };
  }, design);
}
function closeTo(actual, expected, message) { assert.ok(Math.abs(actual - expected) <= .05, `${message}: expected ${expected}, received ${actual}`); }
function aligned(value) {
  closeTo(value.game.left - value.list.left, 16, 'Game select list inset'); closeTo(value.list.right - value.game.right, 16, 'Game select right inset');
  closeTo(value.session.left - value.detail.left, 16, 'Session select detail inset'); closeTo(value.detail.right - value.session.right, 16, 'Session select right inset');
  closeTo(value.heading.left - value.list.left, 16, 'List heading inset'); closeTo(value.recordText.left - value.list.left, 16, 'Record text inset');
  closeTo(value.meta.left - value.detail.left, 16, 'Detail meta inset'); closeTo(value.original.left - value.detail.left, 16, 'Detail source inset');
  assert.deepEqual(value.detailPadding, ['16px', '16px']);
  closeTo(value.label.top + value.label.height / 2, value.action.top + value.action.height / 2, 'Lightweight directory action shares the label row center');
  assert.ok(value.label.right <= value.action.left, 'Directory action must not overlap label');
  assert.ok(value.action.bottom <= value.session.top, 'Directory action must not overlap Select');
  assert.ok(value.session.bottom <= value.status.top, 'Directory status has a separate row');
  assert.ok(value.game.bottom <= value.list.top && value.session.bottom <= value.list.top, 'Filters must stay above body');
  assert.equal(value.documentOverflow, false);
  if (!value.stacked) { closeTo(value.game.top, value.session.top, 'Select top alignment'); closeTo(value.trackRatio, .86 / 1.4, 'Shared filter/body track proportion'); }
}
async function reachable(locator) {
  await locator.scrollIntoViewIfNeeded();
  const value = await locator.evaluate(node => {
    const r = node.getBoundingClientRect(); let clip = { left: 0, top: 0, right: innerWidth, bottom: innerHeight };
    for (let p = node.parentElement; p; p = p.parentElement) { const s = getComputedStyle(p); if (/(auto|scroll|hidden)/.test(s.overflowY + ' ' + s.overflowX)) { const b = p.getBoundingClientRect(); clip = { left: Math.max(clip.left, b.left), top: Math.max(clip.top, b.top), right: Math.min(clip.right, b.right), bottom: Math.min(clip.bottom, b.bottom) }; } }
    return { rect: r.toJSON(), clip, visible: r.left >= clip.left - 1 && r.top >= clip.top - 1 && r.right <= clip.right + 1 && r.bottom <= clip.bottom + 1 };
  }); assert.equal(value.visible, true, JSON.stringify(value)); return value;
}

before(async () => {
  await fs.mkdir(output, { recursive: true });
  const admission = JSON.parse(await fs.readFile(path.join(evidenceRoot, 'admission.json'), 'utf8'));
  beforeSource = Object.fromEntries(await Promise.all([screen, css].map(async name => { const value = await fs.readFile(path.join(evidenceRoot, 'preimage', name), 'utf8'); assert.equal(hash(value), admission.preimage[name].sha256); return [name, value]; })));
  candidateSource = Object.fromEntries(await Promise.all([screen, css].map(async name => [name, await fs.readFile(path.join(root, name), 'utf8')])));
  sourceBefore = await sourceHashes(); fixedBefore = await fixedHealth(); await record('source-before', sourceBefore);
  for (const relative of ['apps/desktop/src', 'apps/desktop/tests/support', 'apps/desktop/tests/live-preview', 'apps/desktop/tsconfig.json', 'apps/desktop/package.json', 'docs/design/first-product-v1/assets']) { const destination = path.join(workspace, relative); await fs.mkdir(path.dirname(destination), { recursive: true }); await fs.cp(path.join(root, relative), destination, { recursive: true }); }
  for (const name of [screen, css]) await fs.writeFile(path.join(workspace, name), beforeSource[name]);
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir'); await fs.symlink(path.join(root, 'node_modules'), path.join(workspace, 'node_modules'), 'dir');
  // Install the existing controlled query adapter in a private development entry.
  // Production source, fixtures, store and the running user preview are untouched.
  const client = path.join(desktop, 'tests/live-preview/client.tsx'), clientSource = await fs.readFile(client, 'utf8'), seam = 'const store = new ApplicationStore(port);';
  assert.equal(clientSource.split(seam).length, 2);
  const supportBootstrap = await fs.readFile(path.join(desktop, 'tests/support/bootstrap.tsx'), 'utf8');
  const wordCardHost = supportBootstrap.slice(supportBootstrap.indexOf('function ControlledWordCardSurface()'), supportBootstrap.indexOf('const reactRoot = createRoot(container);'));
  assert.ok(wordCardHost.includes('<WordCardSurface key={card.cardId} port={port}/>'), 'Reuse the existing real word-card surface host');
  await fs.writeFile(client, "import { installHistoryFixture } from '../support/history-fixture-port';\nimport { useSyncExternalStore } from 'react';\nimport { WordCardSurface } from '../../src/features/language';\n" + clientSource.replace(seam, 'Object.assign(window, { uiHistoryHarness: installHistoryFixture(port) });\n' + seam).replace('reactRoot.render(<WindowRoot role={role} store={store}/>);', wordCardHost + '\nreactRoot.render(<><WindowRoot role={role} store={store}/><ControlledWordCardSurface/></>);'));
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve)); const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = `http://127.0.0.1:${port}`;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', path.join(desktop, 'tests/live-preview/vite.config.ts'), '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', chunk => { serverLog += chunk; }); server.stderr.on('data', chunk => { serverLog += chunk; });
  for (let index = 0; index < 150; index++) { try { if ((await fetch(origin)).ok) break; } catch {} if (index === 149 || server.exitCode !== null) throw new Error('Isolated preview did not start: ' + serverLog); await new Promise(resolve => setTimeout(resolve, 100)); }
  staticServer = createHttpServer(async (request, response) => {
    try { const filename = path.resolve(designRoot, decodeURIComponent(new URL(request.url, 'http://localhost').pathname).replace(/^\/+/, '')); if (!filename.startsWith(designRoot + path.sep)) throw Error('Outside design root'); const type = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.png': 'image/png', '.svg': 'image/svg+xml' }[path.extname(filename)] ?? 'application/octet-stream'; const bytes = await fs.readFile(filename); response.writeHead(200, { 'Content-Type': type }); response.end(bytes); } catch { response.writeHead(404); response.end('Not found'); }
  }); await new Promise(resolve => staticServer.listen(0, '127.0.0.1', resolve)); designOrigin = `http://127.0.0.1:${staticServer.address().port}`;
  ({ browser, identity } = await launchLockedBrowser());
});
after(async () => {
  for (const page of pages) await page.close();
  let browserIdentityVerified = false;
  if (browser) { await verifyBrowserIdentity(identity); browserIdentityVerified = true; await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('exit', resolve)); }
  if (staticServer) await new Promise(resolve => staticServer.close(resolve));
  const fixedAfter = await fixedHealth(), sourceAfter = await sourceHashes();
  const sourceStable = JSON.stringify(sourceAfter) === JSON.stringify(sourceBefore), fixedPreviewStable = fixedAfter?.pid === fixedBefore?.pid, suitePassed = cases.length === 7 && cases.every(value => value.status === 'passed');
  await record('results', { kind: 'real-production-history-layout-only', status: suitePassed && sourceStable && fixedPreviewStable && browserIdentityVerified ? 'passed' : 'failed', suitePassed, sourceStable, fixedPreviewStable, browserIdentityVerified, browser: identity, origin, designOrigin, fixedBefore, fixedAfter, sourceBefore, sourceAfter, cases, scope: 'production WindowRoot/HistoryScreen; existing controlled history query adapter; isolated Vite copy', native: 'not_run', sqlite: 'not_run', pixelAlignment: 'diagnostic_screenshots_only; independently asserted approved source layout geometry', extremeLongSessionPointer: { status: 'passed', priorFailure: 'run-1794041832956', method: 'Real pointer selects All after the extremely long selected session at CSS200; shared source identities are included.' }, product_completion: 'incomplete' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  assert.equal(fixedAfter?.pid ?? null, fixedBefore?.pid ?? null, 'The user preview process must not change'); assert.deepEqual(sourceAfter, sourceBefore, 'Tests must not modify production, fixture or design sources'); console.log('History layout evidence: ' + output);
});

check('paired admitted preimage and candidate retain real production screenshots at both C1 sizes', async () => {
  const facts = [];
  for (const width of [1040, 760]) { const context = await open({ width, height: width === 760 ? 560 : 720 }); facts.push({ phase: 'before', width, value: await geometry(context.page) }); await finish(context, `before-${width}`); }
  for (const name of [screen, css]) await fs.writeFile(path.join(workspace, name), candidateSource[name]);
  for (const width of [1040, 760]) { const context = await open({ width, height: width === 760 ? 560 : 720 }); const value = await geometry(context.page); aligned(value); closeTo(value.navigation.width, width === 1040 ? 190 : 57, 'C1 navigation width'); facts.push({ phase: 'after', width, value }); await finish(context, `after-${width}`); }
  await record('before-after-geometry', facts); return facts;
});

check('independent approved design and production share the 16px alignment and two primary tracks', async () => {
  const facts = [];
  for (const width of [1040, 760]) {
    const page = await browser.newPage({ viewport: { width, height: width === 760 ? 560 : 720 }, deviceScaleFactor: 1 }); pages.add(page);
    await page.goto(`${designOrigin}/${designEntry}?scenario=history&theme=dark&historyState=long`); await page.locator('.layout-history-session-tools').waitFor(); await frame(page);
    const reference = await geometry(page, true); aligned(reference); await page.screenshot({ path: path.join(output, `design-${width}.png`), animations: 'disabled' });
    const context = await open({ width, height: width === 760 ? 560 : 720 }); const candidate = await geometry(context.page); aligned(candidate);
    assert.equal(candidate.stacked, reference.stacked); closeTo(candidate.trackRatio, reference.trackRatio, 'Independent design track ratio');
    facts.push({ width, reference, candidate }); await finish(context, `candidate-reference-${width}`); await page.close(); pages.delete(page);
  } await record('design-comparison', facts); return facts;
});

check('narrow, 200 percent, light and long labels preserve alignment and reachable directory/detail actions', async () => {
  const facts = [];
  for (const options of [{ width: 1040, height: 720, zoom: 2 }, { width: 760, height: 560, zoom: 2 }, { width: 760, height: 560, theme: 'light' }]) {
    const context = await open({ ...options, navigate: false }), { page } = context;
    await page.evaluate(() => { const p = window.liveUiPreview.port, games = structuredClone(p.state.games); games[0].displayName = '名称非常长的游戏 · 日本語の物語 '.repeat(16); p.publish({ games }); });
    await page.getByRole('button', { name: '历史记录', exact: true }).click(); await rows(page).first().waitFor();
    await select(page, '游戏筛选', 'history-game-a'); await page.waitForFunction(() => document.querySelector('[role=combobox][aria-label="游戏筛选"]').dataset.selectValue === 'history-game-a' && document.querySelector('[aria-label="记录详情"]').getAttribute('aria-busy') === 'false');
    await select(page, '会话筛选', 'history-session-a-0'); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 1); await frame(page);
    const sessionEllipsis = await page.getByRole('combobox', { name: '会话筛选', exact: true }).locator('span').first().evaluate(node => ({ overflow: getComputedStyle(node).textOverflow, clipped: node.scrollWidth > node.clientWidth })); assert.deepEqual(sessionEllipsis, { overflow: 'ellipsis', clipped: true });
    aligned(await geometry(page)); await capture(context, `long-session-${options.width}-${options.zoom ?? 1}-${options.theme ?? 'dark'}`);
    await select(page, '会话筛选', ''); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 50); await frame(page);
    const value = await geometry(page); aligned(value);
    const ellipsis = await page.getByRole('combobox', { name: '游戏筛选', exact: true }).locator('span').first().evaluate(node => ({ overflow: getComputedStyle(node).textOverflow, clipped: node.scrollWidth > node.clientWidth })); assert.deepEqual(ellipsis, { overflow: 'ellipsis', clipped: true });
    const targets = [];
    for (const locator of [page.getByRole('combobox', { name: '游戏筛选', exact: true }), page.getByRole('combobox', { name: '会话筛选', exact: true }), page.getByRole('button', { name: '加载更早会话', exact: true }), page.getByRole('button', { name: '加载更早记录', exact: true }), page.getByRole('button', { name: '修正 历史测试引擎', exact: true })]) targets.push(await reachable(locator));
    await page.getByRole('button', { name: '修正 历史测试引擎', exact: true }).click(); const input = page.getByRole('textbox', { name: '人工修正', exact: true }); await input.fill('窄窗与放大仍可编辑。'); await reachable(page.getByRole('button', { name: '取消编辑', exact: true })); await page.getByRole('button', { name: '取消编辑', exact: true }).click();
    facts.push({ options, value, targets }); await finish(context, `reachable-${options.width}-${options.zoom ?? 1}-${options.theme ?? 'dark'}`);
  } await record('reachability', facts); return facts;
});

check('directory and record paging remain independent while loaded selection stays fixed during editing', async () => {
  const context = await open(), { page } = context; assert.equal(await rows(page).count(), 50);
  await page.getByRole('button', { name: '修正 历史测试引擎', exact: true }).click();
  assert.equal(await page.getByRole('button', { name: '加载更早会话', exact: true }).isDisabled(), true);
  assert.equal(await page.getByRole('button', { name: '加载更早记录', exact: true }).isDisabled(), true);
  await page.getByRole('button', { name: '取消编辑', exact: true }).click();
  const recordReads = await page.evaluate(() => window.uiHistoryHarness.recordQueries.length);
  await page.getByRole('button', { name: '加载更早会话', exact: true }).click(); await page.getByText('没有更早会话', { exact: true }).waitFor(); assert.equal(await page.evaluate(() => window.uiHistoryHarness.recordQueries.length), recordReads);
  const sessionReads = await page.evaluate(() => window.uiHistoryHarness.sessionQueries.length);
  await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 100); assert.equal(await page.evaluate(() => window.uiHistoryHarness.sessionQueries.length), sessionReads);
  await rows(page).nth(56).click(); await detailText(page).filter({ hasText: '甲游戏记录 56' }).waitFor();
  await page.getByRole('button', { name: '修正 历史测试引擎', exact: true }).click(); const input = page.getByRole('textbox', { name: '人工修正', exact: true }); await input.fill('第五十六条记录的受控修正。');
  for (const name of ['游戏筛选', '会话筛选']) assert.equal(await page.getByRole('combobox', { name, exact: true }).isDisabled(), true);
  assert.equal(await rows(page).first().isDisabled(), true); assert.equal(await page.getByRole('button', { name: '加载更早记录', exact: true }).isDisabled(), true);
  await page.evaluate(() => window.uiHistoryHarness.correctionResponses.push({ translationId: 'history-translation-56', baseRevision: 0, correction: { revision: 1, text: '第五十六条记录的受控修正。' } }));
  await page.getByRole('button', { name: '保存修正', exact: true }).click(); await input.waitFor({ state: 'hidden' }); await page.locator('[aria-label="记录详情"] p').filter({ hasText: /^第五十六条记录的受控修正。$/ }).waitFor();
  assert.equal(await rows(page).count(), 100); assert.equal(await rows(page).nth(56).getAttribute('aria-pressed'), 'true'); const command = await page.evaluate(() => window.liveUiPreview.port.commands.at(-1).command); assert.equal(command.payload.translationId, 'history-translation-56'); assert.equal(command.payload.baseRevision, 0);
  await finish(context, 'pagination-edit-protection'); return { loaded: 100, selectedId: 'history-unit-56', command };
});

check('scope failure keeps old records readable, directory failure stays separate, retry uses exact requested scope', async () => {
  const context = await open(), { page } = context; await rows(page).nth(4).click();
  await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'records', gameId: 'history-game-b', error: 'controlled records scope failure' }, { kind: 'sessions', gameId: 'history-game-b', error: 'controlled directory scope failure' }));
  await select(page, '游戏筛选', 'history-game-b'); await page.getByRole('alert').filter({ hasText: '新范围加载失败，仍保留上一范围的记录' }).waitFor(); await page.getByText('新范围会话加载失败，上一目录未用于当前筛选。', { exact: true }).waitFor();
  assert.equal(await rows(page).count(), 50); assert.ok((await detailText(page).innerText()).includes('甲游戏记录 4')); assert.equal(await page.getByRole('combobox', { name: '会话筛选', exact: true }).isDisabled(), true); aligned(await geometry(page));
  await capture(context, 'failed-scope-readable'); await page.getByRole('button', { name: '重新加载会话', exact: true }).click(); await page.getByRole('combobox', { name: '会话筛选', exact: true }).waitFor({ state: 'visible' }); await page.waitForFunction(() => !document.querySelector('[role=combobox][aria-label="会话筛选"]').disabled);
  await page.getByRole('button', { name: '重新加载记录', exact: true }).click(); await rows(page).first().filter({ hasText: '乙游戏记录 65' }).waitFor();
  const queries = await page.evaluate(() => ({ records: window.uiHistoryHarness.recordQueries.slice(-2), sessions: window.uiHistoryHarness.sessionQueries.slice(-2) }));
  for (const values of Object.values(queries)) { assert.deepEqual(values.map(query => query.cursor), [null, null]); assert.ok(values.every(query => query.filter.gameId === 'history-game-b')); }
  await finish(context, 'retried-scope'); return queries;
});

check('superseded responses cannot clear current pending scope or replace selected content', async () => {
  const context = await open(), { page } = context;
  await page.evaluate(() => window.uiHistoryHarness.faults.push({ kind: 'records', gameId: 'history-game-a', hold: 'old-a' }, { kind: 'sessions', gameId: 'history-game-a', hold: 'old-a-directory' }, { kind: 'records', gameId: 'history-game-b', hold: 'current-b' }));
  await select(page, '游戏筛选', 'history-game-a'); await page.waitForFunction(() => window.uiHistoryHarness.pending.has('old-a') && window.uiHistoryHarness.pending.has('old-a-directory'));
  await select(page, '游戏筛选', 'history-game-b'); await page.waitForFunction(() => window.uiHistoryHarness.pending.has('current-b'));
  await page.evaluate(() => { window.uiHistoryHarness.release('old-a'); window.uiHistoryHarness.release('old-a-directory'); }); await frame(page);
  assert.equal(await page.locator('[aria-label="可滚动的历史记录"]').getAttribute('aria-busy'), 'true'); assert.ok((await rows(page).first().innerText()).includes('甲游戏记录 0'));
  await page.evaluate(() => window.uiHistoryHarness.release('current-b')); await rows(page).first().filter({ hasText: '乙游戏记录 65' }).waitFor();
  await finish(context, 'late-response-isolation'); return { currentGame: 'history-game-b' };
});

check('long detail and record panes scroll independently and lexical words retain a frozen selected object', async () => {
  const context = await open({ navigate: false }), { page } = context;
  await page.evaluate(() => {
    const p = window.liveUiPreview.port, units = structuredClone(p.state.history.units); units[0].sourceText = '言葉'; units[0].tokens = [{ surface: '言葉', lemma: '言葉', reading: 'ことば', partOfSpeech: '名詞', byteStart: 0, byteEnd: 6, utf16Start: 0, utf16End: 2, known: true }]; units[0].translations[0].text = '长译文独立滚动。\n'.repeat(80); p.publish({ history: { units, nextCursor: null } });
  }); await page.getByRole('button', { name: '历史记录', exact: true }).click(); await rows(page).first().waitFor();
  const facts = await page.evaluate(() => { const detail = document.querySelector('[aria-label="记录详情"]'), list = document.querySelector('[aria-label="可滚动的历史记录"]'), filter = document.querySelector('[role=combobox][aria-label="游戏筛选"]'); const before = filter.getBoundingClientRect().toJSON(); detail.scrollTop = 300; return { scrollable: detail.scrollHeight > detail.clientHeight, detailScroll: detail.scrollTop, listScroll: list.scrollTop, before, after: filter.getBoundingClientRect().toJSON() }; });
  assert.equal(facts.scrollable, true); assert.ok(facts.detailScroll >= 300); assert.equal(facts.listScroll, 0); assert.deepEqual(facts.before, facts.after);
  const reverse = await page.evaluate(() => { const detail = document.querySelector('[aria-label="记录详情"]'), list = document.querySelector('[aria-label="可滚动的历史记录"]'); list.scrollTop = 200; return { listScroll: list.scrollTop, detailScroll: detail.scrollTop }; });
  assert.equal(reverse.listScroll, 200); assert.equal(reverse.detailScroll, facts.detailScroll); facts.reverse = reverse;
  // Owner scrolling intentionally dismisses word cards. Finish returning to the
  // chosen word before opening the stationary frozen surface.
  await page.locator('[aria-label="记录详情"]').getByRole('button', { name: '言葉，读音 ことば', exact: true }).scrollIntoViewIfNeeded(); await frame(page);
  await page.locator('[aria-label="记录详情"]').getByRole('button', { name: '言葉，读音 ことば', exact: true }).click(); await page.getByTestId('controlled-word-card-surface').waitFor(); const frozen = await page.evaluate(() => window.liveUiPreview.port.wordCardSnapshot());
  await page.evaluate(() => { const p = window.liveUiPreview.port; p.publish({ current: { ...structuredClone(p.state.current), sourceText: '游戏已经推进到下一句。' }, commitRevision: p.state.commitRevision + 1 }); }); await frame(page);
  assert.deepEqual(await page.evaluate(() => window.liveUiPreview.port.wordCardSnapshot()), frozen); assert.equal(await page.evaluate(() => window.liveUiPreview.port.wordCardRequests.length), 1);
  await finish(context, 'independent-scroll-fixed-word'); return facts;
});
