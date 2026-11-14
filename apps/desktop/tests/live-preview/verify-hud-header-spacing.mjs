/** HUD header regression: production WindowRoot, controlled transport, independent design source. */
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
const evidenceRoot = path.join(root, '.build/current-client-execution-20261105/hud-header-spacing-20261107');
const output = path.join(evidenceRoot, 'implementation', 'run-' + Date.now());
const workspace = path.join(output, 'workspace');
const desktop = path.join(workspace, 'apps/desktop');
const hudCss = 'apps/desktop/src/windows/hud/styles.module.css';
const designRoot = path.join(root, 'docs/design/first-product-v1');
const parentEntry = 'candidates/guided-service-reading-20261030/index.html';
const composedEntry = 'candidates/hud-header-spacing-20261107/index.html';
const composedCss = 'candidates/hud-header-spacing-20261107/hud-header.css';
const overrideLink = '<link rel="stylesheet" href="' + composedCss + '">';
const sourcePaths = [hudCss, 'apps/desktop/src/windows/hud/HudWindow.tsx', 'apps/desktop/src/features/model-config/ServiceSelector.tsx', 'apps/desktop/src/features/model-config/service-selector.module.css', 'apps/desktop/tests/live-preview/verify-hud-header-spacing.mjs', 'docs/design/first-product-v1/' + parentEntry, 'docs/design/first-product-v1/' + composedEntry, 'docs/design/first-product-v1/' + composedCss];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let browser, identity, server, staticServer, origin, designOrigin, fixedBefore, sourceBefore, candidateCss, beforeCss, serverLog = '';
const cases = [], pages = new Set();
const check = (name, run) => test(name, { timeout: 60000 }, async () => {
  try { const value = await run(); cases.push({ name, status: 'passed', value }); }
  catch (error) { cases.push({ name, status: 'failed', error: error.stack }); throw error; }
});
async function record(name, value) { await fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n'); }
async function sourceHashes() { return Object.fromEntries(await Promise.all(sourcePaths.map(async relative => [relative, hash(await fs.readFile(path.join(root, relative)))]))); }
async function fixedHealth() {
  try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(500) })).json(); }
  catch { return null; }
}
async function frame(page) { await page.evaluate(() => document.fonts.ready); await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function open({ width = 560, height = 290, compact = false, theme = 'dark', empty = false } = {}) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 }); pages.add(page);
  page.setDefaultTimeout(6000);
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${origin}/client.html?role=hud&view=hud&state=${empty ? 'empty' : 'normal'}&theme=${theme}`);
  await page.waitForFunction(() => Boolean(window.liveUiPreview?.port));
  await page.locator('[data-ui-surface="hud"] > header [data-service-selector] > button[aria-busy="false"]').waitFor();
  if (compact) {
    await page.getByRole('button', { name: '切换紧凑布局', exact: true }).click();
    await page.getByRole('button', { name: '切换舒适布局', exact: true }).waitFor();
  }
  await frame(page);
  return { page, errors };
}
async function finish(context, name) {
  await frame(context.page); await context.page.screenshot({ path: path.join(output, name + '.png') });
  assert.deepEqual(context.errors, []); await context.page.close(); pages.delete(context.page);
}
async function geometry(page, design = false) {
  return page.evaluate(design => {
    const hud = document.querySelector(design ? '#hud-window' : '[data-ui-surface="hud"]');
    const header = hud.querySelector('header');
    const selector = hud.querySelector(design ? '.service-hud-trigger' : '[data-service-selector] > button');
    const tools = hud.querySelector(design ? '.hud-window-tools' : 'header > div:last-child');
    const drag = hud.querySelector('[title="拖动浮窗"]');
    const reading = hud.querySelector(design ? '#hud-reading' : '[data-ui-scroll="reading"]');
    const footer = hud.querySelector('footer');
    const box = node => { const rect = node.getBoundingClientRect(); return { x: rect.x, y: rect.y, width: rect.width, height: rect.height, right: rect.right, bottom: rect.bottom }; };
    const h = box(header), s = box(selector);
    const css = getComputedStyle(header);
    const targets = [drag, selector, ...tools.querySelectorAll('button')].map(node => {
      const rect = box(node), hit = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
      return { label: node.getAttribute('aria-label') ?? node.title, ...rect, hit: hit === node || node.contains(hit) };
    });
    return { viewport: { width: innerWidth, height: innerHeight }, hud: box(hud), header: h, selector: s, tools: box(tools), drag: box(drag), footer: box(footer), reading: { ...box(reading), scrollHeight: reading.scrollHeight, clientHeight: reading.clientHeight, scrollTop: reading.scrollTop }, inset: { top: s.y - h.y, bottom: h.bottom - s.bottom }, headerCss: { height: css.height, minHeight: css.minHeight, paddingTop: css.paddingTop, paddingBottom: css.paddingBottom, paddingLeft: css.paddingLeft, paddingRight: css.paddingRight }, targets, documentOverflow: document.documentElement.scrollWidth > innerWidth };
  }, design);
}
function assertHeader(value, inset) {
  assert.equal(value.header.height, 26 + inset * 2);
  assert.equal(value.selector.height, 26);
  assert.equal(value.inset.top, inset); assert.equal(value.inset.bottom, inset);
  assert.equal(value.headerCss.paddingTop, inset + 'px'); assert.equal(value.headerCss.paddingBottom, inset + 'px');
  assert.equal(value.headerCss.paddingLeft, '7px'); assert.equal(value.headerCss.paddingRight, '7px');
  assert.ok(value.drag.width >= 18, 'The visible drag handle retains a usable width');
  assert.equal(value.drag.height, 26);
  assert.ok(value.drag.right <= value.selector.x, 'Drag handle must not overlap selector');
  assert.ok(value.selector.right <= value.tools.x, 'Selector must not overlap right tools');
  assert.ok(value.tools.right <= value.hud.right, 'Tools stay inside HUD');
  assert.equal(value.targets.every(target => target.hit), true, 'Every header target remains hit-testable');
  assert.deepEqual(value.targets.slice(2).map(target => [target.width, target.height]), [[24, 24], [24, 24], [24, 24]]);
  assert.equal(value.documentOverflow, false);
}
async function serviceName(page, displayName) {
  // A saved-service projection is injected through the existing DesignPort,
  // never by modifying the production selector or its rendered DOM.
  await page.evaluate(displayName => {
    const p = window.liveUiPreview.port;
    const source = { kind: 'http', source: { revisionId: 'hud-spacing-service-revision', modelId: 'hud-spacing-model' } };
    p.serviceFixtures.saved = async query => ({ profileGeneration: p.state.profileGeneration, appliedHeadRevision: p.state.configuration.appliedHeadRevision, selection: { requestedScope: query.scope, effectiveScope: query.scope, selectionRevision: 1, effectiveSelectionRevision: 1, main: source, comparisons: [], purposeEnabled: true }, rows: [{ logicalResourceId: 'hud-spacing-resource', source, purpose: query.purpose, displayName, backend: 'http_translation', verification: { kind: 'untested' }, eligibility: { kind: 'selectable' } }] });
    p.publish({ configuration: { ...p.state.configuration, appliedHeadRevision: p.state.configuration.appliedHeadRevision + 1 } });
  }, displayName);
  await page.locator('[data-service-appearance="hud"] strong').filter({ hasText: displayName }).waitFor(); await frame(page);
}

before(async () => {
  await fs.mkdir(output, { recursive: true });
  const admission = JSON.parse(await fs.readFile(path.join(evidenceRoot, 'admission.json'), 'utf8'));
  beforeCss = await fs.readFile(path.join(evidenceRoot, 'preimage', hudCss), 'utf8');
  assert.equal(hash(beforeCss), admission.preimage[hudCss].sha256, 'Before CSS must be the recorded real preimage');
  candidateCss = await fs.readFile(path.join(root, hudCss), 'utf8');
  assert.match(candidateCss, /\.header\{height:38px;min-height:38px;/);
  assert.equal(candidateCss, beforeCss.replace('.header{height:26px;min-height:26px;', '.header{height:38px;min-height:38px;').replace('padding:0 7px;flex:none}.header:active', 'padding:6px 7px;flex:none}.header:active'), 'Only the two admitted header declarations change');
  sourceBefore = await sourceHashes(); fixedBefore = await fixedHealth(); await record('source-before', sourceBefore);
  for (const relative of ['apps/desktop/src', 'apps/desktop/tests/support', 'apps/desktop/tests/live-preview', 'apps/desktop/tsconfig.json', 'apps/desktop/package.json', 'docs/design/first-product-v1/assets']) {
    const destination = path.join(workspace, relative); await fs.mkdir(path.dirname(destination), { recursive: true }); await fs.cp(path.join(root, relative), destination, { recursive: true });
  }
  await fs.writeFile(path.join(workspace, hudCss), beforeCss);
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
  staticServer = createHttpServer(async (request, response) => {
    try {
      const relative = decodeURIComponent(new URL(request.url, 'http://localhost').pathname).replace(/^\/+/, '');
      const filename = path.resolve(designRoot, relative);
      if (!filename.startsWith(designRoot + path.sep)) throw new Error('Outside design root');
      const contentType = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.json': 'application/json' }[path.extname(filename)] ?? 'application/octet-stream';
      const bytes = await fs.readFile(filename);
      response.writeHead(200, { 'Content-Type': contentType }); response.end(bytes);
    } catch { response.writeHead(404); response.end('Not found'); }
  });
  await new Promise(resolve => staticServer.listen(0, '127.0.0.1', resolve)); designOrigin = `http://127.0.0.1:${staticServer.address().port}`;
  ({ browser, identity } = await launchLockedBrowser());
});

after(async () => {
  for (const page of pages) await page.close();
  if (browser) { await verifyBrowserIdentity(identity); await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('exit', resolve)); }
  if (staticServer) await new Promise(resolve => staticServer.close(resolve));
  const fixedAfter = await fixedHealth(), sourceAfter = await sourceHashes();
  await record('results', { kind: 'real-production-hud-header-spacing-only', browser: identity, origin, designOrigin, fixedBefore, fixedAfter, sourceBefore, sourceAfter, cases, existingSharedSelectorRegression: { path: 'apps/desktop/tests/live-preview/verify-manual-ui-alignment.mjs', status: 'not_run_by_this_script', reason: 'Shared manual/session mutation coverage remains in the existing maintained test.' }, nativeDrag: 'not_run; DesignPort request only', nativePackage: 'not_run', wholeWindowPixelAlignment: 'diagnostic_only', product_completion: 'incomplete' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  assert.equal(fixedAfter?.pid ?? null, fixedBefore?.pid ?? null, 'The user preview process must not change');
  assert.deepEqual(sourceAfter, sourceBefore, 'The test must not alter production or design sources');
  console.log('HUD header evidence: ' + output);
});

check('recorded before and real candidate retain paired viewport screenshots and header geometry', async () => {
  const facts = [];
  for (const width of [560, 400]) {
    const context = await open({ width }); const value = await geometry(context.page); assertHeader(value, 0);
    facts.push({ phase: 'before', width, value }); await finish(context, `before-dark-${width}x290`);
  }
  await fs.writeFile(path.join(workspace, hudCss), candidateCss);
  for (const width of [560, 400]) {
    const context = await open({ width });
    await context.page.waitForFunction(() => getComputedStyle(document.querySelector('[data-ui-surface="hud"] > header')).paddingTop === '6px');
    const value = await geometry(context.page); assertHeader(value, 6);
    facts.push({ phase: 'after', width, value }); await finish(context, `after-dark-${width}x290`);
  }
  await record('before-after-geometry', facts); return facts;
});

check('normal, compact, narrow, long-name, empty and light HUD keep 6px header insets', async () => {
  const facts = [];
  for (const compact of [false, true]) for (const width of [560, 400]) for (const variant of ['normal', 'long-name', 'empty', 'light']) {
    const context = await open({ width, compact, empty: variant === 'empty', theme: variant === 'light' ? 'light' : 'dark' });
    if (variant === 'long-name') await serviceName(context.page, '供字幕连续阅读使用的非常长的已保存翻译服务名称 · 日本語サービス');
    if (variant === 'empty') await context.page.getByText('等待游戏原文', { exact: true }).waitFor();
    const value = await geometry(context.page); assertHeader(value, 6);
    if (variant === 'long-name') {
      const ellipsis = await context.page.locator('[data-service-appearance="hud"] strong').evaluate(node => ({ overflow: getComputedStyle(node).textOverflow, clipped: node.scrollWidth > node.clientWidth }));
      assert.deepEqual(ellipsis, { overflow: 'ellipsis', clipped: true });
    }
    if (variant === 'light') assert.equal(await context.page.locator('html').getAttribute('data-theme'), 'light');
    facts.push({ compact, width, variant, value }); await finish(context, `after-${compact ? 'compact' : 'normal'}-${width}-${variant}`);
  }
  await record('state-matrix-geometry', facts); return { combinations: facts.length };
});

check('selector opens the shared sheet and header tools avoid the drag port while handle requests drag', async () => {
  const context = await open(); const { page } = context;
  const before = await page.evaluate(() => window.liveUiPreview.port.nativeDragRequests);
  await page.locator('[data-service-appearance="hud"] > button').click();
  const sheet = page.getByRole('dialog', { name: '选择翻译服务', exact: true }); await sheet.waitFor();
  await sheet.getByText('还没有已保存的服务。先添加并保存配置，再从这里明确选用。', { exact: false }).waitFor();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.nativeDragRequests), before);
  await page.screenshot({ path: path.join(output, 'after-shared-service-sheet.png') });
  await sheet.locator('footer').getByRole('button', { name: '关闭', exact: true }).click(); await sheet.waitFor({ state: 'hidden' });
  await page.getByRole('button', { name: '取消置顶', exact: true }).click();
  await page.getByRole('button', { name: '始终置顶', exact: true }).waitFor();
  await page.getByRole('button', { name: '切换紧凑布局', exact: true }).click();
  await page.getByRole('button', { name: '切换舒适布局', exact: true }).waitFor();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.nativeDragRequests), before);
  const drag = page.locator('[data-ui-surface="hud"] > header [title="拖动浮窗"]'); await drag.hover(); await page.mouse.down(); await page.mouse.move(120, 80); await page.mouse.up();
  assert.equal(await page.evaluate(() => window.liveUiPreview.port.nativeDragRequests), before + 1, 'This only proves a request to the native port, not native movement');
  const commands = await page.evaluate(() => window.liveUiPreview.port.commands.map(envelope => envelope.command));
  assert.deepEqual(commands.filter(command => command.kind === 'set_hud_preference').map(command => command.payload), [{ preference: 'always_on_top', enabled: false }, { preference: 'compact', enabled: true }]);
  await finish(context, 'after-header-actions'); return { dragRequests: 1, nativeMovement: 'not_run' };
});

check('long reading content scrolls independently while the header and footer stay fixed', async () => {
  const facts = [];
  for (const compact of [false, true]) for (const width of [560, 400]) {
    const context = await open({ width, compact }); const { page } = context;
    await page.evaluate(() => {
      const p = window.liveUiPreview.port, unit = structuredClone(p.state.current);
      unit.unitId += '-long'; unit.sourceText = Array.from({ length: 30 }, (_, index) => `第${index + 1}行、図書館で彼女と待ち合わせをした。`).join('\n'); unit.tokens = []; unit.translations = [];
      p.readingUnitFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = structuredClone(unit);
      p.publish({ current: unit, history: { units: [unit], nextCursor: null }, commitRevision: p.state.commitRevision + 1 });
    });
    await page.getByTestId('source-text').filter({ hasText: '第30行' }).waitFor(); await frame(page);
    const before = await geometry(page); assert.ok(before.reading.scrollHeight > before.reading.clientHeight);
    const reading = page.locator('[data-ui-scroll="reading"]'); await reading.hover(); await page.mouse.wheel(0, 900);
    await page.waitForFunction(() => document.querySelector('[data-ui-scroll="reading"]').scrollTop > 0);
    const after = await geometry(page); assertHeader(after, 6);
    assert.deepEqual(after.header, before.header); assert.deepEqual(after.footer, before.footer); assert.deepEqual(after.selector, before.selector);
    assert.equal(await page.evaluate(() => window.scrollY), 0);
    facts.push({ compact, width, before, after }); await finish(context, `after-scroll-${compact ? 'compact' : 'normal'}-${width}`);
  }
  await record('scroll-geometry', facts); return { combinations: facts.length };
});

check('independent guided composition preserves parent bytes and matches 6px vertical spacing', async () => {
  const parentBytes = await fs.readFile(path.join(designRoot, parentEntry), 'utf8');
  const composedBytes = await fs.readFile(path.join(designRoot, composedEntry), 'utf8');
  assert.equal(composedBytes.replace(overrideLink, ''), parentBytes, 'The new design entry inherits exact guided bytes plus its last stylesheet');
  assert.ok(composedBytes.indexOf(overrideLink) < composedBytes.indexOf('</head>'));
  assert.equal(composedBytes.slice(composedBytes.indexOf(overrideLink) + overrideLink.length, composedBytes.indexOf('</head>')), '', 'Override is the last stylesheet');
  const facts = [];
  for (const phase of ['before', 'after']) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 }); pages.add(page);
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await page.goto(`${designOrigin}/${phase === 'before' ? parentEntry : composedEntry}?scenario=reading&theme=dark`);
    const hud = page.locator('#hud-window'); await hud.waitFor(); await hud.locator('.service-hud-trigger').waitFor();
    // Adapt only the separate design desktop's HUD bounds to the client frame.
    // Its header/selector CSS and DOM continue to come from the independent source.
    await hud.evaluate(node => Object.assign(node.style, { position: 'fixed', left: '0px', top: '0px', right: 'auto', bottom: 'auto', transform: 'none', width: '560px', height: '290px', maxWidth: 'none', zIndex: '9999' }));
    await frame(page); const value = await geometry(page, true); assertHeader(value, phase === 'before' ? 0 : 6);
    assert.equal(value.hud.width, 560); assert.equal(value.hud.height, 290);
    await hud.screenshot({ path: path.join(output, `design-${phase}-560x290.png`) });
    assert.deepEqual(errors, []); facts.push({ phase, value }); await page.close(); pages.delete(page);
  }
  await record('independent-design-geometry', { parentSha256: hash(parentBytes), composedSha256: hash(composedBytes), overrideSha256: hash(await fs.readFile(path.join(designRoot, composedCss))), facts, comparison: 'Header vertical geometry only; separate source and client whole-window PNGs are diagnostic, not an approved pixel baseline.' });
  return { beforeInset: facts[0].value.inset, afterInset: facts[1].value.inset };
});
