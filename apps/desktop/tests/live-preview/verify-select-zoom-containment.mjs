/** Real shared Select: preserve CSS zoom typography while React Aria positions in viewport pixels. */
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
const evidenceRoot = path.join(root, '.build/current-client-execution-20261105/layout-refinement-implementation-20261107');
const output = path.join(evidenceRoot, 'select-zoom', 'run-' + Date.now()), workspace = path.join(output, 'workspace'), desktop = path.join(workspace, 'apps/desktop');
const shared = ['apps/desktop/src/ui/primitives/Select.tsx', 'apps/desktop/src/ui/primitives/controls.module.css'];
const tracked = [...shared, 'apps/desktop/src/features/history/HistoryScreen.tsx', 'apps/desktop/src/features/history/styles.module.css', 'apps/desktop/tests/support/history-fixture-port.ts', 'apps/desktop/tests/live-preview/verify-select-zoom-containment.mjs'];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let browser, identity, server, origin, sourceBefore, fixedBefore, candidate, serverLog = '';
const cases = [], pages = new Set();
const check = (name, run) => test(name, { timeout: 90000 }, async () => { try { const value = await run(); cases.push({ name, status: 'passed', value }); } catch (error) { cases.push({ name, status: 'failed', error: error.stack }); throw error; } });
const record = (name, value) => fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n');
const sourceHashes = async () => Object.fromEntries(await Promise.all(tracked.map(async name => [name, hash(await fs.readFile(path.join(root, name)))])));
async function fixedHealth() { try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(500) })).json(); } catch { return null; } }
async function frame(page) { await page.evaluate(() => document.fonts.ready); await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function capture(context, name) { await frame(context.page); await context.page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' }); assert.deepEqual(context.errors, []); }
async function finish(context, name) { await capture(context, name); await context.page.close(); pages.delete(context.page); }
async function newPage(width, height) { const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 }); pages.add(page); page.setDefaultTimeout(7000); const errors = []; page.on('pageerror', error => errors.push(error.message)); return { page, errors }; }
async function select(page, name, value) { await page.getByRole('combobox', { name, exact: true }).click(); const option = page.getByRole('option'); const index = await option.evaluateAll((nodes, value) => nodes.findIndex(node => node.dataset.selectValue === value), value); assert.ok(index >= 0); await option.nth(index).click(); }
async function history({ width = 1040, zoom = 2, long = true } = {}) {
  const context = await newPage(width, width === 760 ? 560 : 720), { page } = context;
  await page.goto(`${origin}/client.html?role=main&view=manual&state=normal&theme=dark`); await page.waitForFunction(() => window.uiHistoryHarness && window.liveUiPreview?.port);
  await page.evaluate(({ zoom, long }) => { document.documentElement.style.zoom = String(zoom); if (long) { const p = window.liveUiPreview.port, games = structuredClone(p.state.games); games[0].displayName = '名称非常长的游戏 · 日本語の物語 '.repeat(16); p.publish({ games }); } }, { zoom, long });
  await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.locator('[aria-label="历史筛选与记录"] button[aria-pressed]').first().waitFor();
  await select(page, '游戏筛选', 'history-game-a'); await frame(page); await select(page, '会话筛选', 'history-session-a-0'); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 1); await frame(page); return context;
}
async function menuFacts(page) { return page.evaluate(() => {
  const box = node => node ? node.getBoundingClientRect().toJSON() : null;
  const facts = node => { if (!node) return null; const css = getComputedStyle(node); return { rect: box(node), clientHeight: node.clientHeight, scrollHeight: node.scrollHeight, scrollTop: node.scrollTop, clientWidth: node.clientWidth, scrollWidth: node.scrollWidth, css: { zoom: css.zoom, fontSize: css.fontSize, lineHeight: css.lineHeight, maxHeight: css.maxHeight, textOverflow: css.textOverflow, whiteSpace: css.whiteSpace }, style: node.getAttribute('style') }; };
  const popup = document.querySelector('[data-select-panel]'), list = document.querySelector('[role=listbox]'), options = [...document.querySelectorAll('[role=option]')];
  return { viewport: { width: innerWidth, height: innerHeight }, zoom: getComputedStyle(document.documentElement).zoom, host: facts(document.querySelector('[data-select-portal]')), popup: facts(popup), list: facts(list), options: options.slice(0, 4).map(node => ({ value: node.dataset.selectValue, textLength: node.textContent.length, ...facts(node) })), focus: document.activeElement?.getAttribute('data-select-value'), documentOverflow: document.documentElement.scrollWidth > innerWidth, hostCount: document.querySelectorAll('[data-select-portal]').length };
}); }
function contained(facts, zoom) {
  assert.ok(facts.popup.rect.top >= 12 * zoom - 1 && facts.popup.rect.bottom <= facts.viewport.height - 12 * zoom + 1, 'Popover must fit vertical viewport with the retained safety inset: ' + JSON.stringify(facts));
  assert.ok(facts.popup.rect.left >= 12 * zoom - 1 && facts.popup.rect.right <= facts.viewport.width - 12 * zoom + 1, 'Popover must fit horizontal viewport');
  assert.ok(facts.list.rect.top >= facts.popup.rect.top && facts.list.rect.bottom <= facts.popup.rect.bottom, 'List must be contained rather than clipped by the popup');
  assert.equal(facts.documentOverflow, false); assert.equal(facts.hostCount, 1);
}
async function fixture({ width = 1040, zoom = 1 } = {}) { const context = await newPage(width, width === 760 ? 560 : 720); await context.page.goto(`${origin}/select-zoom.html?zoom=${zoom}`); await context.page.getByRole('combobox', { name: '缩放选择', exact: true }).waitFor(); await frame(context.page); return context; }

before(async () => {
  await fs.mkdir(output, { recursive: true }); sourceBefore = await sourceHashes(); fixedBefore = await fixedHealth(); await record('source-before', sourceBefore);
  const admission = JSON.parse(await fs.readFile(path.join(evidenceRoot, 'admission.json'), 'utf8'));
  candidate = Object.fromEntries(await Promise.all(shared.map(async name => [name, await fs.readFile(path.join(root, name), 'utf8')])));
  for (const relative of ['apps/desktop/src', 'apps/desktop/tests/support', 'apps/desktop/tests/live-preview', 'apps/desktop/tsconfig.json', 'apps/desktop/package.json', 'docs/design/first-product-v1/assets']) { const destination = path.join(workspace, relative); await fs.mkdir(path.dirname(destination), { recursive: true }); await fs.cp(path.join(root, relative), destination, { recursive: true }); }
  for (const name of shared) { const bytes = await fs.readFile(path.join(evidenceRoot, 'preimage', name)); assert.equal(hash(bytes), admission.preimage[name].sha256); await fs.writeFile(path.join(workspace, name), bytes); }
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir'); await fs.symlink(path.join(root, 'node_modules'), path.join(workspace, 'node_modules'), 'dir');
  const directory = path.join(desktop, 'tests/live-preview'), client = path.join(directory, 'client.tsx'), original = await fs.readFile(client, 'utf8'), seam = 'const store = new ApplicationStore(port);'; assert.equal(original.split(seam).length, 2);
  await fs.writeFile(client, "import { installHistoryFixture } from '../support/history-fixture-port';\n" + original.replace(seam, 'Object.assign(window, { uiHistoryHarness: installHistoryFixture(port) });\n' + seam));
  await fs.writeFile(path.join(directory, 'select-zoom.html'), '<!doctype html><html lang="zh-CN" data-theme="dark"><head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="root"></div><script type="module" src="/select-zoom.tsx"></script></body></html>');
  await fs.writeFile(path.join(directory, 'select-zoom.tsx'), `import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Select } from '../../src/ui/primitives/Select';
import '../../src/ui/reset.css';
document.documentElement.style.zoom = new URLSearchParams(location.search).get('zoom') ?? '1';
document.documentElement.style.overflow = 'hidden'; document.body.style.overflow = 'hidden';
function Fixture() { const [value, setValue] = useState('first'); return <div><div style={{position:'fixed',left:16,top:16,width:220}}><Select label="禁用选择" disabled value="first" onChange={() => { throw Error('Disabled Select changed'); }} options={[{value:'first',label:'禁止更改'}]}/></div><div data-local-zoom style={{position:'fixed',left:16,bottom:16,width:'calc(100% - 32px)'}}><Select label="缩放选择" value={value} onChange={setValue} options={[{value:'first',label:'第一项'},{value:'disabled',label:'不可选项',disabled:true},{value:'long',label:'名称非常长的游戏 · 日本語の物語 '.repeat(16)},{value:'last',label:'最后一项'}]}/><output aria-label="选择结果">{value}</output></div></div>; }
const root=createRoot(document.getElementById('root')!); root.render(<StrictMode><Fixture/></StrictMode>); Object.assign(window,{unmountSelectFixture:()=>root.unmount()});
`);
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve)); const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = `http://127.0.0.1:${port}`;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', path.join(directory, 'vite.config.ts'), '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] }); server.stdout.on('data', value => { serverLog += value; }); server.stderr.on('data', value => { serverLog += value; });
  for (let index = 0; index < 150; index++) { try { if ((await fetch(origin)).ok) break; } catch {} if (index === 149 || server.exitCode !== null) throw Error('Private Vite failed: ' + serverLog); await new Promise(resolve => setTimeout(resolve, 100)); }
  ({ browser, identity } = await launchLockedBrowser());
});
after(async () => {
  for (const page of pages) await page.close(); let browserIdentityVerified = false; if (browser) { await verifyBrowserIdentity(identity); browserIdentityVerified = true; await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('exit', resolve)); }
  const sourceAfter = await sourceHashes(), fixedAfter = await fixedHealth(), sourceStable = JSON.stringify(sourceBefore) === JSON.stringify(sourceAfter), fixedPreviewStable = fixedBefore?.pid === fixedAfter?.pid, suitePassed = cases.length === 4 && cases.every(value => value.status === 'passed');
  await record('results', { kind: 'shared-select-css-zoom-viewport-containment', status: suitePassed && sourceStable && fixedPreviewStable && browserIdentityVerified ? 'passed' : 'failed', suitePassed, sourceStable, fixedPreviewStable, browserIdentityVerified, sourceBefore, sourceAfter, fixedBefore, fixedAfter, browser: identity, cases, scope: 'shared production Select and real HistoryScreen with existing controlled adapter; private Vite source copy', native: 'not_run', wholeProduct: 'incomplete' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog); assert.equal(sourceStable, true); assert.equal(fixedPreviewStable, true); console.log('Select zoom evidence: ' + output);
});

check('admitted preimage reproduces negative portal coordinates before the candidate fixes the same pointer path', async () => {
  const facts = [];
  for (const width of [1040, 760]) { const context = await history({ width }); await context.page.getByRole('combobox', { name: '会话筛选', exact: true }).click(); await frame(context.page); const before = await menuFacts(context.page); assert.ok(before.popup.rect.top < 0); assert.ok(before.options[0].rect.bottom < 0); facts.push({ phase: 'before', width, facts: before }); await finish(context, `before-pointer-${width}`); }
  for (const name of shared) await fs.writeFile(path.join(workspace, name), candidate[name]);
  for (const width of [1040, 760]) { const context = await history({ width }); const { page } = context; await page.getByRole('combobox', { name: '会话筛选', exact: true }).click(); await frame(page); const after = await menuFacts(page); contained(after, 2); await capture(context, `after-contained-menu-${width}`); const all = page.locator('[role=option][data-select-value=""]'); await all.scrollIntoViewIfNeeded(); await frame(page); const target = await menuFacts(page); assert.ok(target.options[0].rect.top >= target.list.rect.top && target.options[0].rect.bottom <= target.list.rect.bottom); await all.click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 50); facts.push({ phase: 'after', width, facts: after, target }); await finish(context, `after-pointer-${width}`); }
  await record('before-after-pointer', facts); return facts;
});

check('normal and narrow history at both zooms retain long options and real pointer navigation', async () => {
  const facts = [];
  for (const width of [1040, 760]) for (const zoom of [1, 2]) for (const long of [false, true]) {
    const context = await history({ width, zoom, long }), { page } = context;
    await page.getByRole('combobox', { name: '会话筛选', exact: true }).click(); await frame(page); const opened = await menuFacts(page); contained(opened, zoom);
    const all = page.locator('[role=option][data-select-value=""]'); await all.scrollIntoViewIfNeeded(); await frame(page); const target = await menuFacts(page); assert.equal(target.options[0].rect.height, 30 * zoom, 'Short option actual size preserves CSS zoom'); assert.ok(target.options[0].rect.top >= target.list.rect.top && target.options[0].rect.bottom <= target.list.rect.bottom); if (long) assert.ok(target.options[1].textLength > 300, 'Long text remains intact');
    await all.click(); await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 50); assert.equal(await page.locator('[data-select-portal]').count(), 0);
    await page.getByRole('combobox', { name: '会话筛选', exact: true }).click();
    const last = page.locator('[role=option][data-select-value="history-session-a-49"]'); await last.scrollIntoViewIfNeeded(); await last.click();
    await page.waitForFunction(() => document.querySelectorAll('[aria-label="历史筛选与记录"] button[aria-pressed]').length === 1);
    assert.equal(await page.getByRole('combobox', { name: '会话筛选', exact: true }).getAttribute('data-select-value'), 'history-session-a-49');
    facts.push({ width, zoom, long, opened, target }); await finish(context, `history-${width}-${zoom}-${long ? 'long' : 'short'}`);
  } await record('history-matrix', facts); return { combinations: facts.length };
});

check('shared Select keeps disabled, keyboard, mouse hover and selection semantics in the contained menu', async () => {
  const facts = [];
  for (const width of [1040, 760]) for (const zoom of [1, 2]) {
    const context = await fixture({ width, zoom }), { page } = context, trigger = page.getByRole('combobox', { name: '缩放选择', exact: true });
    assert.equal(await page.getByRole('combobox', { name: '禁用选择', exact: true }).isDisabled(), true); assert.equal(await page.locator('[data-select-portal]').count(), 0);
    await trigger.focus(); await page.keyboard.press('ArrowDown'); await page.getByRole('listbox').waitFor(); await frame(page); contained(await menuFacts(page), zoom);
    await page.keyboard.press('Home'); assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-select-value')), 'first'); await page.keyboard.press('ArrowDown'); assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-select-value')), 'long', 'Keyboard skips disabled option'); await page.keyboard.press('Enter'); await page.getByRole('status', { name: '选择结果', exact: true }).filter({ hasText: 'long' }).waitFor();
    await trigger.click(); const disabled = page.locator('[role=option][data-select-value=disabled]'); await disabled.scrollIntoViewIfNeeded(); const disabledRect = await disabled.boundingBox(); await page.mouse.click(disabledRect.x + disabledRect.width / 2, disabledRect.y + Math.min(10, disabledRect.height / 2)); assert.equal(await trigger.getAttribute('data-select-value'), 'long'); assert.equal(await page.getByRole('listbox').count(), 1);
    const first = page.locator('[role=option][data-select-value=first]'); await first.hover(); assert.equal(await trigger.getAttribute('data-select-value'), 'long', 'Hover changes focus only'); assert.equal(await first.getAttribute('data-focused'), 'true'); await first.click(); assert.equal(await trigger.getAttribute('data-select-value'), 'first'); assert.equal(await page.locator('[data-select-portal]').count(), 0);
    await trigger.click(); await page.keyboard.press('End'); assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-select-value')), 'last'); await page.keyboard.press('Enter'); assert.equal(await trigger.getAttribute('data-select-value'), 'last'); facts.push({ width, zoom, selected: 'last' }); await finish(context, `controls-${width}-${zoom}`);
  } return facts;
});

check('open portals update on CSS zoom and resize, then clean up on close, reopen and StrictMode unmount', async () => {
  const context = await fixture(), { page } = context, trigger = page.getByRole('combobox', { name: '缩放选择', exact: true });
  await trigger.click(); await frame(page); const normal = await menuFacts(page); contained(normal, 1);
  await page.evaluate(() => document.documentElement.style.zoom = '2'); await frame(page); const zoomed = await menuFacts(page); contained(zoomed, 2); assert.equal(zoomed.options[0].rect.height, normal.options[0].rect.height * 2);
  await page.setViewportSize({ width: 760, height: 560 }); await frame(page); const resized = await menuFacts(page); contained(resized, 2);
  await page.keyboard.press('Escape'); assert.equal(await page.locator('[data-select-portal]').count(), 0); await trigger.click(); await frame(page); contained(await menuFacts(page), 2);
  await page.evaluate(() => window.unmountSelectFixture()); assert.equal(await page.locator('[data-select-panel], [data-select-portal]').count(), 0); await page.evaluate(() => document.documentElement.style.zoom = '1'); await frame(page); assert.equal(await page.locator('[data-select-portal]').count(), 0);
  await finish(context, 'lifecycle-cleaned'); return { normal, zoomed, resized };
});
