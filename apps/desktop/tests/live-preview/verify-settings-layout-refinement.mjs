/** Approved settings spacing on real components. Controlled ports are not Native/DB acceptance. */
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
const output = path.join(evidenceRoot, 'settings', 'run-' + Date.now());
const desktop = path.join(output, 'workspace/apps/desktop');
const owned = ['apps/desktop/src/features/preferences/PreferencesScreen.tsx', 'apps/desktop/src/features/preferences/GeneralSettings.tsx', 'apps/desktop/src/features/preferences/styles.module.css', 'apps/desktop/src/windows/settings/SettingsWindow.tsx', 'apps/desktop/src/windows/settings/styles.module.css', 'apps/desktop/tests/live-preview/verify-settings-layout-refinement.mjs'];
const protectedPaths = ['apps/desktop/src/ui/primitives/Controls.tsx', 'apps/desktop/src/ui/primitives/controls.module.css', 'apps/desktop/tests/support/design-port.ts', 'docs/design/first-product-v1/candidates/layout-refinements-20261107/settings.html', 'docs/design/first-product-v1/candidates/layout-refinements-20261107/history-settings.css'];
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const results = [], browserErrors = [];
let browser, identity, server, origin, serverLog = '', sourceBefore, fixedBefore;
async function identities() { return Object.fromEntries(await Promise.all([...owned, ...protectedPaths].map(async file => [file, digest(await fs.readFile(path.join(root, file)))]))); }
async function health() {
  try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(1000) })).json(); }
  catch { return null; }
}
async function record(file, value) { await fs.writeFile(path.join(output, file + '.json'), JSON.stringify(value, null, 2) + '\n'); }
function check(name, run) { test(name, { timeout: 90000 }, async () => {
  try { results.push({ name, status: 'passed', facts: await run() }); }
  catch (error) { results.push({ name, status: 'failed', error: error.stack }); throw error; }
}); }
before(async () => {
  await fs.mkdir(desktop, { recursive: true });
  sourceBefore = await identities(); fixedBefore = await health(); await record('source-before', sourceBefore);
  for (const relative of ['src', 'tests/support', 'package.json']) await fs.cp(path.join(root, 'apps/desktop', relative), path.join(desktop, relative), { recursive: true });
  await fs.cp(path.join(desktop, 'src'), path.join(desktop, 'before-src'), { recursive: true });
  for (const file of owned.filter(file => file.includes('/src/'))) {
    const preimage = path.join(evidenceRoot, 'preimage', file);
    const destination = path.join(desktop, 'before-src', file.split('/src/')[1]);
    await fs.copyFile(preimage, destination);
  }
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir');
  await fs.symlink(path.join(root, 'node_modules'), path.join(output, 'workspace/node_modules'), 'dir');
  await fs.writeFile(path.join(desktop, 'layout-harness.tsx'), `import React from 'react';
import {createRoot} from 'react-dom/client';
import {ApplicationStore} from './src/bridge/projections/application-store';
import {DesignPort} from './tests/support/design-port';
import './src/ui/reset.css';
const options=JSON.parse(new URLSearchParams(location.search).get('options')||'{}');
document.documentElement.dataset.windowRole='settings';
const {WindowRoot}=options.before?await import('./before-src/app/WindowRoot'):await import('./src/app/WindowRoot');
const port=new DesignPort('reading');
port.state.preferences={...port.state.preferences,scopeKey:'global',theme:options.theme??'dark',segmentationEnabled:true,showRuby:true,showPos:true};
port.state.games=[{gameId:'settings-layout-game',displayName:'设置回归游戏',originalTitle:null,sourceLanguage:'ja',coverAssetRef:null,rowRevision:1,target:{environmentKind:'unresolved',environmentKey:'',bottleLocator:null,launchPath:null,canonicalExePath:null,targetPathKey:null,workingDirectory:null,arguments:[],architecture:null,verificationState:'unresolved'}}];
port.loginStartupFixture={status:'not_registered',canChange:true,errorCode:null,detail:'登录启动由系统回执确认。'};
port.serviceFixtures.saved=async query=>({profileGeneration:port.state.profileGeneration,appliedHeadRevision:port.state.configuration.appliedHeadRevision,selection:{requestedScope:query.scope,effectiveScope:{kind:'global'},selectionRevision:0,effectiveSelectionRevision:0,main:null,comparisons:[],purposeEnabled:false},rows:[]});
const store=new ApplicationStore(port);window.settingsLayout={port,store};
createRoot(document.getElementById('root')).render(<WindowRoot role="settings" store={store}/>);`);
  await fs.writeFile(path.join(desktop, 'index.html'), '<!doctype html><html lang="zh-CN"><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><style>html,body,#root{margin:0;width:100%;height:100%;overflow:hidden}body{background:#161827}</style><div id="root"></div><script type="module" src="/layout-harness.tsx"></script></html>');
  const config = path.join(desktop, 'layout-vite.config.mjs');
  await fs.writeFile(config, `import react from '@vitejs/plugin-react';import fs from 'node:fs/promises';import path from 'node:path';
const design=${JSON.stringify(path.join(root, 'docs/design/first-product-v1'))};
export default{root:${JSON.stringify(desktop)},plugins:[react(),{name:'read-only-frozen-reference',configureServer(server){server.middlewares.use('/reference',async(req,res,next)=>{try{const file=path.resolve(design,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));if(!file.startsWith(design+path.sep))return next();const bytes=await fs.readFile(file);res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.jpg':'image/jpeg','.svg':'image/svg+xml','.json':'application/json'})[path.extname(file)]||'application/octet-stream');res.end(bytes);}catch{next();}});}}],cacheDir:${JSON.stringify(path.join(output, 'vite-cache'))},server:{host:'127.0.0.1',fs:{allow:[${JSON.stringify(path.join(output, 'workspace'))},${JSON.stringify(root)}]}}};`);
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = `http://127.0.0.1:${port}`;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', config, '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', chunk => serverLog += chunk); server.stderr.on('data', chunk => serverLog += chunk);
  for (let index = 0; index < 150; index++) {
    try { if ((await fetch(origin)).ok) break; } catch {}
    if (index === 149 || server.exitCode !== null) throw Error('Private Vite failed: ' + serverLog);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  ({ browser, identity } = await launchLockedBrowser());
}, { timeout: 120000 });
after(async () => {
  if (browser) { await verifyBrowserIdentity(identity); await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('close', resolve)); }
  const sourceAfter = await identities(), fixedAfter = await health();
  await record('results', { kind: 'real-settings-layout-controlled-port', browser: identity, sourceBefore, sourceAfter, fixedBefore, fixedAfter, results, browserErrors, native: 'not_run', database: 'not_run', product_completion: 'incomplete', limits: ['Scoped preferences and native startup are controlled transport responses; no persisted database or OS registration claim.', 'Screenshots compare approved local spacing and reachability; extra existing product scope/status rows remain visible.'] });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  assert.deepEqual(sourceAfter, sourceBefore, 'Owned and protected inputs must stay stable');
  assert.deepEqual(fixedAfter, fixedBefore, 'The user preview process must stay unchanged');
  assert.deepEqual(browserErrors, []); console.log('Settings layout evidence: ' + output);
});
async function settle(page) { await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function capture(page, name) { await page.evaluate(() => document.fonts.ready); await settle(page); await page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' }); }
async function open(options = {}) {
  const page = await browser.newPage({ viewport: { width: options.width ?? 1000, height: options.height ?? 720 }, deviceScaleFactor: 1 });
  page.setDefaultTimeout(8000); page.on('pageerror', error => browserErrors.push(error.message));
  await page.goto(`${origin}/?options=${encodeURIComponent(JSON.stringify(options))}`);
  await page.getByRole('navigation', { name: '设置分类' }).getByRole('button', { name: '通用', exact: true }).click();
  await page.locator('[data-ui-page="general-settings"]').getByText('阅读偏好已保存', { exact: false }).waitFor();
  await page.getByRole('switch', { name: '登录后打开客户端', exact: true }).waitFor();
  if (options.zoom) { await page.evaluate(zoom => { document.documentElement.style.zoom = String(zoom); }, options.zoom); await settle(page); }
  return page;
}
const general = page => page.locator('[data-ui-page="general-settings"]');
const lexical = page => general(page).locator('section').filter({ has: page.getByRole('heading', { name: '内置分词与词典', exact: true }) });
const toggle = (page, name) => general(page).getByRole('switch', { name, exact: true });
async function setToggle(page, name, selected) {
  const input = toggle(page, name);
  if (await input.isChecked() !== selected) await input.locator('xpath=ancestor::label').click();
  assert.equal(await input.isChecked(), selected);
}
const save = page => general(page).getByRole('button', { name: '保存阅读偏好', exact: true });
const discard = page => general(page).getByRole('button', { name: '放弃修改', exact: true });
async function chooseScope(page, label) {
  await general(page).getByRole('combobox', { name: '阅读偏好作用范围', exact: true }).click();
  await page.getByRole('option', { name: label, exact: true }).click();
  await settle(page);
}
async function metrics(page) {
  return general(page).evaluate(node => {
    const zoom = Number(document.documentElement.style.zoom || 1), rect = item => { const b = item.getBoundingClientRect(); return { x: b.x / zoom, y: b.y / zoom, right: b.right / zoom, bottom: b.bottom / zoom, width: b.width / zoom, height: b.height / zoom }; };
    const rows = [...node.querySelectorAll('[data-form-row]')].map(row => ({ padding: getComputedStyle(row).padding, columns: getComputedStyle(row).gridTemplateColumns, overflow: row.scrollWidth > row.clientWidth }));
    const switches = [...node.querySelectorAll('section')].find(section => section.querySelector('h2')?.textContent === '内置分词与词典');
    const toggles = [...switches.querySelectorAll('[data-switch-label]')].map(label => { const root = label.parentElement, track = label.nextElementSibling, css = getComputedStyle(root), r = rect(root), t = rect(track), l = rect(label); return { padding: css.padding, gap: css.columnGap, separator: getComputedStyle(root.parentElement).borderTopWidth, root: r, label: l, track: t, leftInset: l.x - r.x, rightInset: r.right - t.right, overlap: l.right > t.x }; });
    const groups = [...node.querySelectorAll(':scope>div>section,:scope>div>fieldset>section')].map(section => ({ title: section.querySelector('h2')?.textContent, margin: getComputedStyle(section).marginBottom, headingGap: getComputedStyle(section.querySelector('h2')).marginBottom, cardPadding: getComputedStyle(section.querySelector('h2+div')).padding, noteGap: section.querySelector(':scope>p') ? getComputedStyle(section.querySelector(':scope>p')).marginTop : null }));
    const sidebar = document.querySelector('[data-client-container="settings-a1000x720"]>aside');
    return { rows, toggles, groups, sidebarWidth: rect(sidebar).width, horizontalOverflow: node.scrollWidth > node.clientWidth, documentOverflow: document.documentElement.scrollWidth > innerWidth, scroll: rect(node.querySelector('[data-ui-scroll]')), footer: rect(node.querySelector('footer')) };
  });
}
async function reference(options) {
  const page = await browser.newPage({ viewport: { width: options.width, height: options.height }, deviceScaleFactor: 1 });
  await page.goto(`${origin}/reference/candidates/layout-refinements-20261107/settings.html?client=c1`);
  await page.locator('.layout-toggle-row').first().waitFor();
  await page.evaluate(theme => { const select = document.querySelector('#review-theme'); select.value = theme; select.dispatchEvent(new Event('change', { bubbles: true })); }, options.theme); await settle(page);
  if (options.zoom) await page.evaluate(zoom => { document.documentElement.style.zoom = String(zoom); }, options.zoom);
  const facts = await page.evaluate(() => ({ rows: [...document.querySelectorAll('.ls-row:not(.layout-toggle-row)')].map(node => ({ padding: getComputedStyle(node).padding })), toggles: [...document.querySelectorAll('.layout-toggle-row')].map(node => ({ padding: getComputedStyle(node).padding, gap: getComputedStyle(node).columnGap })), groups: [...document.querySelectorAll('.ls-section')].map(node => ({ margin: getComputedStyle(node).marginBottom })), sidebarWidth: document.querySelector('.ls-sidebar')?.getBoundingClientRect().width / Number(document.documentElement.style.zoom || 1) }));
  await capture(page, `reference-${options.width}-${options.theme}-${options.zoom ?? 1}-top`);
  await page.locator('.layout-toggle-row').first().scrollIntoViewIfNeeded(); await capture(page, `reference-${options.width}-${options.theme}-${options.zoom ?? 1}-switches`); await page.close();
  return facts;
}

check('SETTINGS-LAYOUT-01-approved-source-and-real-components-normal-narrow-light-dark-zoom', async () => {
  const facts = [];
  for (const options of [{ width: 1000, height: 720, theme: 'dark' }, { width: 760, height: 560, theme: 'dark' }, { width: 1000, height: 720, theme: 'light' }, { width: 760, height: 560, theme: 'light' }, { width: 1000, height: 720, theme: 'dark', zoom: 2 }, { width: 760, height: 560, theme: 'light', zoom: 2 }]) {
    const expected = await reference(options), page = await open(options), actual = await metrics(page);
    await record(`geometry-${options.width}-${options.theme}-${options.zoom ?? 1}`, { expected, actual });
    assert.equal(actual.sidebarWidth, options.width === 760 ? 180 : 198);
    assert.equal(actual.horizontalOverflow, false); assert.equal(actual.documentOverflow, false);
    assert.ok(actual.rows.every(row => !row.overflow));
    assert.ok(actual.rows.every(row => row.padding === expected.rows[0].padding), 'Ordinary row padding must match the frozen source at every width');
    for (const [index, row] of actual.toggles.entries()) {
      assert.equal(row.padding, expected.toggles[index].padding); assert.ok(row.leftInset >= 14 - .1); assert.ok(row.rightInset >= 14 - .1);
      assert.equal(row.gap, expected.toggles[index].gap); assert.equal(row.separator, index ? '1px' : '0px');
      assert.equal(row.track.width, 32); assert.equal(row.track.height, 19); assert.equal(row.overlap, false);
      assert.ok(row.label.bottom <= row.root.bottom - 12 + .1); assert.ok(row.label.y >= row.root.y + 12 - .1);
    }
    assert.ok(actual.groups.every(group => group.cardPadding === '0px' && group.headingGap === '8px'));
    assert.ok(actual.groups.slice(0, -1).every(group => group.margin === expected.groups[0].margin));
    assert.equal(actual.groups.find(group => group.title === '内置分词与词典').noteGap, '7px');
    const name = `${options.width}-${options.theme}-${options.zoom ?? 1}`;
    await capture(page, `client-${name}-top`);
    await toggle(page, '可点击日语分词').scrollIntoViewIfNeeded(); await capture(page, `client-${name}-switches`);
    const last = general(page).locator('[data-form-row]').last(); await last.scrollIntoViewIfNeeded();
    await general(page).locator('[data-ui-scroll]').hover(); await page.mouse.wheel(0, 10000); await settle(page);
    const lastBox = await last.boundingBox(), viewport = await general(page).locator('[data-ui-scroll]').boundingBox();
    assert.ok(lastBox.y + lastBox.height <= viewport.y + viewport.height + 1 && lastBox.y + lastBox.height > viewport.y, 'The final line must be reachable by the content scroll, including rows taller than a zoomed viewport');
    for (const button of [discard(page), save(page)]) {
      await button.focus(); const box = await button.boundingBox();
      assert.ok(box.y >= 0 && box.y + box.height <= options.height, 'Every footer action must remain fully reachable at 200%');
      assert.ok(box.x >= 0 && box.x + box.width <= options.width, 'Footer actions must remain inside the client width');
    }
    await capture(page, `client-${name}-bottom`); facts.push({ options, expected, actual }); await page.close();
  }
  return facts;
});
check('SETTINGS-LAYOUT-02-admitted-before-image-proves-direct-switch-regression', async () => {
  const before = await open({ before: true }), after = await open();
  const old = await metrics(before), current = await metrics(after);
  assert.ok(old.toggles.every(row => row.leftInset < 1 && row.rightInset < 1));
  assert.ok(current.toggles.every(row => row.leftInset >= 14 && row.rightInset >= 14));
  await toggle(before, '可点击日语分词').scrollIntoViewIfNeeded(); await capture(before, 'before-admitted-switches');
  await toggle(after, '可点击日语分词').scrollIntoViewIfNeeded(); await capture(after, 'after-switches');
  await before.close(); await after.close(); return { old, current };
});
check('SETTINGS-LAYOUT-03-POS-retains-value-and-ruby-remains-independent-through-save-and-rollback', async () => {
  const page = await open();
  await setToggle(page, '可点击日语分词', false); assert.equal(await toggle(page, '显示词性标记').isDisabled(), true); assert.equal(await toggle(page, '显示词性标记').isChecked(), true);
  assert.equal(await toggle(page, '显示日语注音').isDisabled(), false); await setToggle(page, '显示日语注音', false);
  assert.equal(await general(page).getByRole('combobox', { name: '阅读偏好作用范围', exact: true }).isDisabled(), true);
  await save(page).click(); await page.waitForFunction(() => window.settingsLayout.port.scopedPreferenceSaves.has('global'));
  await save(page).waitFor({ state: 'visible' }); await settle(page);
  const saved = await page.evaluate(() => window.settingsLayout.port.scopedPreferenceSaves.get('global').saved);
  assert.equal(saved.segmentationEnabled, false); assert.equal(saved.showPos, true); assert.equal(saved.showRuby, false);
  await setToggle(page, '可点击日语分词', true); assert.equal(await toggle(page, '显示词性标记').isDisabled(), false); assert.equal(await toggle(page, '显示词性标记').isChecked(), true);
  await setToggle(page, '显示日语注音', true); await discard(page).click(); await settle(page);
  assert.equal(await toggle(page, '可点击日语分词').isChecked(), false); assert.equal(await toggle(page, '显示日语注音').isChecked(), false); assert.equal(await toggle(page, '显示词性标记').isChecked(), true);
  await capture(page, 'independent-ruby-and-preserved-POS'); await page.close(); return saved;
});
check('SETTINGS-LAYOUT-04-failed-save-keeps-draft-and-game-scope-inheritance-stays-explicit', async () => {
  const page = await open(); const game = await page.evaluate(() => window.settingsLayout.port.state.games[0]);
  await chooseScope(page, `仅此游戏 · ${game.displayName}`); await general(page).getByText('此游戏继承全局默认', { exact: true }).waitFor();
  await setToggle(page, '显示日语注音', false); await page.evaluate(() => window.settingsLayout.port.scopedPreferenceSaveResponses.push({ error: '受控保存失败' }));
  await save(page).click(); await general(page).getByRole('alert').filter({ hasText: '受控保存失败' }).waitFor();
  assert.equal(await toggle(page, '显示日语注音').isChecked(), false); assert.equal(await save(page).isEnabled(), true);
  assert.equal(await page.evaluate(() => window.settingsLayout.port.scopedPreferenceSaves.size), 0);
  await capture(page, 'failed-save-kept-game-draft'); await save(page).click();
  await page.waitForFunction(key => window.settingsLayout.port.scopedPreferenceSaves.has(key), `game:${game.gameId}`); await settle(page);
  const saved = await page.evaluate(key => window.settingsLayout.port.scopedPreferenceSaves.get(key).saved, `game:${game.gameId}`);
  assert.equal(saved.overrides.showRuby, true); assert.equal(saved.showRuby, false);
  // Prepare the explicit scope-query projection. The convenience DesignPort
  // inheritedDraft retains local scalar values; Rust owns the real merge.
  // This response supplies the actual query contract without editing that adapter.
  await chooseScope(page, '全局默认');
  await page.evaluate(key => { const p = window.settingsLayout.port, view = p.controlledScope(key); view.inheritedDraft = { ...structuredClone(view.global), scopeKey: key, overrides: {} }; p.scopedPreferenceResponses.push({ scopeKey: key, value: view }); }, `game:${game.gameId}`);
  await chooseScope(page, `仅此游戏 · ${game.displayName}`);
  await general(page).getByRole('button', { name: '恢复继承全局', exact: true }).click();
  assert.equal(await save(page).isEnabled(), true); assert.equal(await toggle(page, '显示日语注音').isChecked(), true);
  await save(page).click(); await page.waitForFunction(key => Object.keys(window.settingsLayout.port.scopedPreferenceSaves.get(key).saved.overrides).length === 0, `game:${game.gameId}`);
  await settle(page); await chooseScope(page, '全局默认'); assert.equal(await toggle(page, '显示日语注音').isChecked(), true);
  await page.close(); return { scopeKey: saved.scopeKey, overrides: saved.overrides };
});
check('SETTINGS-LAYOUT-05-history-entry-capture-and-startup-confirmation-keep-real-authorities', async () => {
  const page = await open();
  assert.equal(await general(page).locator('[data-history-policy="always"]').count(), 1);
  assert.equal(await general(page).getByRole('switch', { name: /历史/ }).count(), 0);
  await general(page).getByRole('button', { name: '查看阅读历史', exact: true }).click();
  assert.equal(await page.evaluate(() => window.settingsLayout.port.mainPageRequests[0].intent), 'history');
  await page.evaluate(() => { const p = window.settingsLayout.port; p.loginStartupSetResponses.push({ value: { status: 'requires_approval', canChange: true, errorCode: null, detail: '受控等待系统批准' }, holdKey: 'startup-write' }); p.loginStartupResponses.push({ value: { status: 'requires_approval', canChange: true, errorCode: null, detail: '受控等待系统批准' } }); });
  await toggle(page, '登录后打开客户端').locator('xpath=ancestor::label').click(); await page.waitForFunction(() => window.settingsLayout.port.loginStartupPending.has('startup-write'));
  assert.equal(await toggle(page, '登录后打开客户端').isDisabled(), true);
  assert.equal(await toggle(page, '登录后打开客户端').isChecked(), false, 'A pending write must not render its intention as success');
  await page.evaluate(() => window.settingsLayout.port.releaseLoginStartup('startup-write'));
  await general(page).locator('[data-login-startup-status="requires_approval"]').waitFor();
  await general(page).getByText('等待系统批准', { exact: true }).waitFor();
  const queries = await page.evaluate(() => window.settingsLayout.port.loginStartupQueries); assert.ok(queries >= 2);
  const captureSwitch = toggle(page, '新台词自动加入当前会话'); const oldCapture = await captureSwitch.isChecked(); await setToggle(page, '新台词自动加入当前会话', !oldCapture);
  await page.waitForFunction(() => window.settingsLayout.port.commands.some(envelope => envelope.command.kind === 'set_capture_paused'));
  assert.equal(await page.evaluate(() => window.settingsLayout.port.commands.find(envelope => envelope.command.kind === 'set_capture_paused').command.payload.paused), oldCapture);
  assert.equal(await page.evaluate(() => window.settingsLayout.port.commands.some(envelope => envelope.command.kind === 'save_scoped_preferences')), false);
  const navigation = page.getByRole('navigation', { name: '设置分类' }); await navigation.getByRole('button', { name: 'OCR引擎', exact: true }).click();
  await page.getByText('本期窗口预览独立于 OCR', { exact: false }).waitFor(); assert.equal(await page.getByText('也不请求屏幕录制权限', { exact: false }).count(), 0);
  await capture(page, 'OCR-scope-preserves-preview-permission'); await page.close(); return { startupQueries: queries, capturePaused: oldCapture };
});
