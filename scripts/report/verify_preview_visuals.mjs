/** Finite C35 current-source/production-React comparison. Never a native or all-state adapter. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { createHash, randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import nodeAssert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const desktop = path.join(root, 'apps/desktop');
const argument = key => process.argv[process.argv.indexOf(key) + 1];
const out = path.resolve(root, process.argv.includes('--output') ? argument('--output') : `.build/visual-correction-20261104/qa/${Date.now()}-${randomUUID()}`);
const nonce = process.argv.includes('--nonce') ? argument('--nonce') : randomUUID();
const buildRoot = await fs.realpath(path.join(root, '.build'));
if (!nonce || !out.startsWith(path.join(root, '.build') + path.sep) || fsSync.existsSync(out)) throw Error('Fresh repository .build output and nonce required');
let existingParent = path.dirname(out);
while (!fsSync.existsSync(existingParent)) existingParent = path.dirname(existingParent);
const realParent = await fs.realpath(existingParent);
if (realParent !== buildRoot && !realParent.startsWith(buildRoot + path.sep)) throw Error('Output parent must stay inside the repository .build tree');
await fs.mkdir(out, { recursive: true });
const require = createRequire(path.join(root, 'package.json'));
const { chromium } = require('playwright-core'), { PNG } = require('pngjs'), pixelmatch = require('pixelmatch').default;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
let assertions = 0;
const assert = new Proxy(nodeAssert, { apply(fn, self, args) { assertions++; return Reflect.apply(fn, self, args); }, get(fn, key) { const value = fn[key]; return typeof value === 'function' ? (...args) => { assertions++; return Reflect.apply(value, fn, args); } : value; } });
async function files(dir) { const result = []; for (const item of await fs.readdir(dir, { withFileTypes: true })) { const file = path.join(dir, item.name); if (item.isDirectory()) result.push(...await files(file)); else if (item.isFile()) result.push(file); } return result.sort(); }
const audited = [...await files(path.join(desktop, 'src')), ...await files(path.join(desktop, 'tests')), ...['package.json', 'package-lock.json', 'apps/desktop/package.json', 'apps/desktop/package-lock.json', 'apps/desktop/tsconfig.json', 'apps/desktop/vite.config.ts', 'apps/desktop/src-tauri/tauri.conf.json', 'scripts/report/verify_preview_visuals.mjs'].map(file => path.join(root, file))];
const sourceSha256 = Object.fromEntries(await Promise.all(audited.sort().map(async file => [path.relative(root, file), sha(await fs.readFile(file))])));
// Raw input content makes the exact source used for the execution independently inspectable.
await fs.writeFile(path.join(out, 'source-inputs.json'), JSON.stringify(Object.fromEntries(await Promise.all(audited.map(async file => [path.relative(root, file), { sha256: sourceSha256[path.relative(root, file)], encoding: 'utf8', content: await fs.readFile(file, 'utf8') }]))), null, 2) + '\n');
const approvalFile = 'docs/development/design-approval-complete-20261031.json';
const approvalBytes = await fs.readFile(path.join(root, approvalFile)), approval = JSON.parse(approvalBytes);
assert.equal(approval.design_source_count, 61); assert.equal(approval.status, 'approved_by_user');
for (const [file, expected] of Object.entries(approval.design_sources)) assert.equal(sha(await fs.readFile(path.join(root, file))), expected, `Approved source drift: ${file}`);
await fs.writeFile(path.join(out, 'design-approval.json'), approvalBytes);
const historicalReceiptFile = process.argv.includes('--historical-receipt') ? argument('--historical-receipt') : '.build/logic-correction-20261104/frozen-historical-replay-attempt3/20261104T003748.549831Z-69069db2e1-product-ui-capture-v4/capture/verification.json';
const historicalBytes = await fs.readFile(path.resolve(root, historicalReceiptFile)), historicalReceipt = JSON.parse(historicalBytes);
const historicalClassifications = {
  'session-current-body': 'retired_session_tabs', 'session-current-extraction': 'retired_session_tabs', 'session-current-hook-events': 'retired_session_tabs', 'preview-off': 'retired_manual_preview_off', 'target-review-preview-off': 'retired_manual_preview_off',
  'library-populated': 'historical_cover_and_retired_footer_geometry', 'target-review-preview-live': 'historical_preview_order_and_structure', 'target-review': 'manual_off_premise_mismatch_preserved_as_failed', 'add-game': 'historical_program_record_and_arguments_explanation', 'library-batch': 'historical_program_record_and_arguments_explanation',
  'preview-live': 'historical_preview_pixels', 'preview-paused': 'historical_preview_pixels', 'preview-permission': 'historical_terminal_frame_policy_and_pixels', 'preview-unavailable': 'historical_terminal_frame_policy_and_pixels', 'preview-exited': 'historical_terminal_frame_policy_and_pixels', 'session-hook-events': 'historical_hook_diagnostic_pixels_current_alignment_not_inferred',
};
const historicalInventory = { receipt: historicalReceiptFile, sha256: sha(historicalBytes), kind: 'retained_historical_v4_only', total: historicalReceipt.captures.length, statusCounts: {}, states: historicalReceipt.captures.map(row => ({ id: row.id, status: row.status, historicalClassification: historicalClassifications[row.id] ?? 'historical_unchanged_pass', currentSourceVerdict: 'not_inferred_from_historical_receipt', ratio: row.ratio ?? null, reason: row.reason ?? null, reference: row.reference ?? null, variants: row.variants ?? null })), supersedesCurrentSource: false };
for (const row of historicalInventory.states) historicalInventory.statusCounts[row.status] = (historicalInventory.statusCounts[row.status] ?? 0) + 1;
await fs.writeFile(path.join(out, 'historical-37-inventory.json'), JSON.stringify(historicalInventory, null, 2) + '\n');
const frameRoot = path.join(root, 'docs/design/first-product-v1/acceptance-v4');
const frameInput = JSON.parse(await fs.readFile(path.join(frameRoot, 'preview-frame-input.json'))), frameBytes = await fs.readFile(path.join(frameRoot, frameInput.file));
assert.equal(sha(frameBytes), frameInput.sha256); assert.equal(sha(await fs.readFile(path.join(root, frameInput.source))), frameInput.sourceSha256);
await fs.writeFile(path.join(out, 'shared-frame.jpg'), frameBytes);
const frame = { ...frameInput, base64: frameBytes.toString('base64') };
const title = 'サクラノ詩 10th Anniversary edition';
const longTitle = 'サクラノ詩 Re:Graphic 10th Anniversary edition ― 特別限定パッケージ・追加ストーリー収録版（日本語）';
const sharedTargetFacts = { gameTitle: title, windowTitle: title, executablePath: 'C:\\Games\\SakuraNoUta10th\\sakuranouta10th.exe', windowsPid: 6200, environmentKey: 'crossover:Sakura-no-Uta-10th', bottleLabel: 'Sakura-no-Uta-10th', macosWindowId: '101', origin: 'candidate' };
const clock = Date.UTC(2026, 9, 18, 10, 24, 8);
const csp = JSON.parse(await fs.readFile(path.join(desktop, 'src-tauri/tauri.conf.json'), 'utf8')).app.security.csp;
const variants = [
  { id: 'narrow-dark-live', width: 282, theme: 'dark', state: 'live' },
  { id: 'wide-dark-live', width: 708, theme: 'dark', state: 'live' },
  { id: 'boundary519-dark-live', width: 519, theme: 'dark', state: 'live' },
  { id: 'boundary520-dark-live', width: 520, theme: 'dark', state: 'live' },
  { id: 'content519-dark-live', width: 521, theme: 'dark', state: 'live' },
  { id: 'content520-dark-live', width: 522, theme: 'dark', state: 'live' },
  { id: 'narrow-light-live', width: 282, theme: 'light', state: 'live' },
  { id: 'wide-light-live', width: 708, theme: 'light', state: 'live' },
  { id: 'narrow-dark-paused', width: 282, theme: 'dark', state: 'paused' },
  { id: 'narrow-dark-stale', width: 282, theme: 'dark', state: 'stale' },
  { id: 'narrow-dark-permission', width: 282, theme: 'dark', state: 'permission_denied' },
  { id: 'narrow-dark-closed', width: 282, theme: 'dark', state: 'target_closed' },
  { id: 'narrow-dark-unavailable', width: 282, theme: 'dark', state: 'unavailable' },
  { id: 'narrow-dark-long-title', width: 282, theme: 'dark', state: 'live', title: longTitle },
  { id: 'wide-light-long-title', width: 708, theme: 'light', state: 'live', title: longTitle },
  { id: 'narrow-dark-zoom200', width: 282, theme: 'dark', state: 'live', zoom: 2 },
  { id: 'narrow-light-long-title-zoom200', width: 282, theme: 'light', state: 'live', zoom: 2, title: longTitle },
  { id: 'narrow-dark-reduced', width: 282, theme: 'dark', state: 'live', reduced: true },
];
const checks = [], comparisons = [], detailComparisons = [], builds = [], pageErrors = [], requestViolations = [], cspViolations = [], maps = [];
const startedAt = new Date().toISOString();
let browser, server, origin, browserVersion;
async function artifact(file) { const bytes = await fs.readFile(path.join(out, file)); return { path: file, sha256: sha(bytes), bytes: bytes.length }; }
async function json(file, data) { await fs.writeFile(path.join(out, file), JSON.stringify(data, null, 2) + '\n'); return artifact(file); }
async function check(id, execute) { const start = assertions; try { const observations = await execute(); checks.push({ id, status: 'passed', assertionCount: assertions - start, observations }); console.log(`${id}: passed`); } catch (error) { checks.push({ id, status: 'failed', assertionCount: assertions - start, error: error.message, stack: error.stack }); console.log(`${id}: failed: ${error.message}`); } }
async function build(args, label) { const command = [process.execPath, ...args], child = spawn(command[0], command.slice(1), { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] }); let stdout = '', stderr = ''; child.stdout.on('data', data => { stdout += data; }); child.stderr.on('data', data => { stderr += data; }); const exitCode = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); }); const log = await json(`${label}.json`, { command, cwd: desktop, startedAt, finishedAt: new Date().toISOString(), exitCode, stdout, stderr }); builds.push({ command, exitCode, log }); assert.equal(exitCode, 0, stderr); }
function initialPort(port, input) {
  const projection = port.prepareSessionPreview('current-c35-candidate', 'f'.repeat(64), input.state);
  Object.assign(projection, { attemptedAtMs: String(input.clock), capturedAtMs: ['live', 'paused', 'stale'].includes(input.state) ? String(input.clock) : null, contentChangedAtMs: ['live', 'paused', 'stale'].includes(input.state) ? String(input.clock) : null, permissionCheckedAtMs: String(input.clock), permission: input.state === 'permission_denied' ? 'denied' : 'granted', errorCode: input.state === 'unavailable' ? 'window_not_on_screen' : null, detail: 'Current-source comparison explicit controlled transport diagnostic.' });
  port.previewProjections.set(projection.leaseId, structuredClone(projection));
  if (['live', 'paused', 'stale'].includes(input.state)) port.emitPreview({ projection, frame: { leaseId: projection.leaseId, generation: projection.generation, sequence: '1', targetIdentityHash: projection.targetIdentityHash, capturedAtMs: String(input.clock), width: input.frame.width, height: input.frame.height, contentHash: input.frame.sha256, encoding: 'jpeg', payload: [...Uint8Array.from(atob(input.frame.base64), character => character.charCodeAt(0))] } });
  if (input.state === 'permission_denied') port.previewCapabilityFixture = { supported: true, permission: 'denied', errorCode: 'screen_capture_permission', detail: 'Controlled denied permission' };
  port.reviewedCandidatePreviewFixture = { candidateId: 'current-c35-target', projection };
  port.state.preferences.theme = input.theme; port.state.preferences.transparencyMode = input.reduced ? 'reduced' : 'material';
}
async function pageBase(input, kind) {
  const page = await browser.newPage({ viewport: { width: 1600, height: 1100 }, deviceScaleFactor: 1, colorScheme: input.theme, reducedMotion: 'reduce', locale: 'zh-CN', timezoneId: 'Asia/Shanghai' }); page.setDefaultTimeout(8000);
  page.on('pageerror', error => pageErrors.push({ kind, id: input.id, message: error.message }));
  await page.route('**/*', async route => { const url = new URL(route.request().url()); if (url.origin !== origin && !['data:', 'blob:'].includes(url.protocol)) { requestViolations.push({ kind, id: input.id, url: url.href }); return route.abort(); } return route.continue(); });
  await page.addInitScript(({ clock, zoom }) => { const OriginalDate = Date; window.Date = class extends OriginalDate { constructor(...args) { super(...(args.length ? args : [clock])); } static now() { return clock; } }; window.addEventListener('securitypolicyviolation', event => { (window.__c35CspViolations ??= []).push({ blockedURI: event.blockedURI, directive: event.effectiveDirective }); }); window.__c35Zoom = zoom; }, { clock, zoom: input.zoom ?? 1 });
  if (input.reduced) { const session = await page.context().newCDPSession(page); await session.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-transparency', value: 'reduce' }] }); }
  return page;
}
async function candidate(input) {
  const p = await pageBase(input, 'production');
  await p.addInitScript({ content: `window.__reviewedPreviewInputs = ${JSON.stringify({ title: input.title ?? title, detail: 'Sakura-no-Uta-10th · sakuranouta10th.exe', identityDetail: 'design-window-101 · PID 6200', width: input.width, targetFacts: { ...sharedTargetFacts, gameTitle: input.title ?? title, windowTitle: input.title ?? title } })}; window.__initializeControlledPort = port => (${initialPort.toString()})(port, ${JSON.stringify({ ...input, clock, frame })});` });
  const response = await p.goto(origin + '/tests/visual/index.html?role=main&scenario=empty&reviewedSurface=preview'); assert.equal(await response.headerValue('content-security-policy'), csp);
  const element = p.getByRole('region', { name: '运行窗口预览', exact: true }); await element.waitFor();
  await p.waitForFunction(state => document.querySelector('[data-preview-state]')?.dataset.previewState === state, input.state);
  await p.evaluate(input => { document.documentElement.style.zoom = String(input.zoom ?? 1); }, input);
  await settle(p);
  return { p, element };
}
async function reference(input) {
  if (input.state === 'stale') return { blocked: 'Approved source has no stale mode. Candidate is captured; no invented reference or unavailable substitution.' };
  const p = await pageBase(input, 'independent_approved_source');
  await p.goto(origin + '/source/docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=first-run&theme=' + input.theme);
  await p.waitForFunction(() => window.LunaDesign && window.LunaReviewGaps && document.getElementById('review-preview'));
  const mode = { live: input.title === longTitle ? 'long_title' : 'available', paused: 'available', permission_denied: 'permission', target_closed: 'exited', unavailable: 'unavailable' }[input.state];
  // Choose the terminal design mode before this preview is ever rendered.
  await p.locator('#review-preview').selectOption(mode);
  await p.evaluate(({ input, frame, clock, title, sharedTargetFacts }) => {
    const app = window.LunaDesign, gaps = window.LunaReviewGaps;
    gaps.previews.clear(); app.state.session.gameId = 'sakura'; app.state.session.status = 'connected'; app.state.session.runtimeTarget = { hostWindowId: sharedTargetFacts.macosWindowId, title: input.title ?? title, environment: sharedTargetFacts.environmentKey, bottle: sharedTargetFacts.bottleLabel, exe: sharedTargetFacts.executablePath.split('\\').at(-1), targetPath: sharedTargetFacts.executablePath, windowsPid: sharedTargetFacts.windowsPid, fingerprint: 'f'.repeat(64) };
    app.state.current = { ...(app.state.current ?? {}), scene: `data:image/jpeg;base64,${frame.base64}` };
    const background = document.createElement('div'); background.dataset.independentPreviewUnderlay = ''; Object.assign(background.style, { position: 'fixed', inset: '0', zIndex: '1000', background: 'rgb(2, 12, 49) url(/source/docs/design/first-product-v1/assets/wallpaper-frame.png) center / cover no-repeat' }); document.body.append(background);
    const host = document.createElement('div'); host.id = 'independent-preview-host'; Object.assign(host.style, { position: 'fixed', left: '24px', top: '24px', width: `${input.width}px`, zIndex: '1001' });
    host.innerHTML = gaps.previewHTML(app, app.state.session.runtimeTarget, { selection: true }); document.body.append(host);
    if (input.state === 'paused') { gaps.previews.get(app.state.session.runtimeTarget.hostWindowId).paused = true; host.innerHTML = gaps.previewHTML(app, app.state.session.runtimeTarget, { selection: true }); }
    document.documentElement.style.zoom = String(input.zoom ?? 1); document.documentElement.dataset.transparency = input.reduced ? 'reduced' : 'material';
    // The data/time/frame are shared inputs; source HTML, CSS and pixels are untouched.
    window.__c35ReferenceInputs = { mode: input.state, clock, frameSha256: frame.sha256 };
  }, { input, frame, clock, title, sharedTargetFacts });
  const element = p.locator('#independent-preview-host > section'); await element.waitFor(); await settle(p); return { p, element };
}
async function settle(p) { await p.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.querySelectorAll('[data-preview-state] img,#independent-preview-host img')].map(image => image.decode())); await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); }); await p.mouse.move(1500, 1000); }
async function geometry(element) { return element.evaluate(node => { const rect = element => { const r = element.getBoundingClientRect(), style = getComputedStyle(element); return { x: r.x, y: r.y, width: r.width, height: r.height, clientWidth: element.clientWidth, clientHeight: element.clientHeight, scrollWidth: element.scrollWidth, scrollHeight: element.scrollHeight, display: style.display, gridTemplateColumns: style.gridTemplateColumns, gridTemplateRows: style.gridTemplateRows, fontFamily: style.fontFamily, fontSize: style.fontSize, lineHeight: style.lineHeight, backgroundColor: style.backgroundColor, backdropFilter: style.backdropFilter, materialInsetBlur: style.getPropertyValue('--material-inset-blur').trim(), materialInsetSaturation: style.getPropertyValue('--material-inset-saturation').trim(), objectFit: style.objectFit }; }; const children = [...node.children].length === 1 ? [...node.firstElementChild.children] : [...node.children]; return { surface: rect(node), layout: rect(node.firstElementChild), children: children.map(rect), images: [...node.querySelectorAll('img')].map(image => ({ ...rect(image), naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight, alt: image.alt })), buttons: [...node.querySelectorAll('button')].map(button => ({ ...rect(button), text: button.textContent, label: button.getAttribute('aria-label'), disabled: button.disabled })), text: node.innerText, theme: document.documentElement.dataset.theme, transparency: document.documentElement.dataset.transparency, dpr: devicePixelRatio, zoom: getComputedStyle(document.documentElement).zoom }; }); }
async function compare(id, expected, actual) {
  const a = PNG.sync.read(await fs.readFile(path.join(out, expected.path))), b = PNG.sync.read(await fs.readFile(path.join(out, actual.path)));
  const width = Math.max(a.width, b.width), height = Math.max(a.height, b.height), left = new PNG({ width, height }), right = new PNG({ width, height }), diff = new PNG({ width, height });
  PNG.bitblt(a, left, 0, 0, a.width, a.height, 0, 0); PNG.bitblt(b, right, 0, 0, b.width, b.height, 0, 0);
  const options = { threshold: .1, includeAA: true, maxRatio: 0 };
  const pixels = pixelmatch(left.data, right.data, diff.data, width, height, { threshold: options.threshold, includeAA: options.includeAA });
  const difference = `${id}.diff.png`; await fs.writeFile(path.join(out, difference), PNG.sync.write(diff));
  // Independently reread stored pixels and recompute; no cached acceptance result.
  const recomputed = pixelmatch(PNG.sync.read(PNG.sync.write(left)).data, PNG.sync.read(PNG.sync.write(right)).data, null, width, height, { threshold: .1, includeAA: true }); assert.equal(recomputed, pixels);
  return { status: pixels === 0 && a.width === b.width && a.height === b.height ? 'passed' : 'failed', options, expectedSize: [a.width, a.height], actualSize: [b.width, b.height], pixels, recomputedPixels: recomputed, ratio: pixels / (width * height), diff: await artifact(difference), dimensionPadding: 'comparison canvas only; original PNGs unchanged' };
}
async function ownership(p) { return p.evaluate(() => { const port = window.uiHarness; return { starts: structuredClone(port.previewRequests), controls: structuredClone(port.previewControls), attaches: structuredClone(port.previewAttaches), detaches: structuredClone(port.previewDetaches), active: [...port.previewListeners.values()].map(({ projection }) => ({ consumerId: projection.consumerId, leaseId: projection.leaseId, generation: projection.generation, targetIdentityHash: projection.targetIdentityHash })) }; }); }

try {
  await build([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck-tests');
  const built = path.join(out, 'controlled-build');
  await build([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(desktop, 'tests/visual/vite.config.ts'), '--outDir', built, '--sourcemap'], 'component-build');
  for (const file of (await files(built)).filter(file => file.endsWith('.js.map'))) {
    const data = JSON.parse(await fs.readFile(file, 'utf8')); assert.equal(data.sources.length, data.sourcesContent.length);
    const production = []; for (let index = 0; index < data.sources.length; index++) { const resolved = path.resolve(path.dirname(file), data.sources[index]); if (resolved.startsWith(path.join(desktop, 'src') + path.sep)) { const bytes = await fs.readFile(resolved, 'utf8'); assert.equal(data.sourcesContent[index], bytes, `Built raw source mismatch ${resolved}`); production.push({ file: path.relative(root, resolved), sha256: sha(bytes), sourceMapIndex: index }); } }
    maps.push({ file: path.relative(out, file), sha256: sha(await fs.readFile(file)), productionSources: production });
  }
  for (const component of ['WindowPreview.tsx', 'PreviewDetails.tsx']) assert.ok(maps.some(map => map.productionSources.some(item => item.file.endsWith('/' + component))), `${component} raw production contents in actual build`);
  server = http.createServer(async (req, res) => { try { const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname), isSource = pathname.startsWith('/source/'), base = isSource ? root : built, relative = isSource ? pathname.slice('/source'.length) : pathname, file = path.resolve(base, '.' + relative); if (!file.startsWith(base + path.sep)) throw Error('Outside root'); const bytes = await fs.readFile(file), extension = path.extname(file); res.writeHead(200, { 'content-type': ({ '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css', '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml', '.json': 'application/json' })[extension] ?? 'application/octet-stream', ...(!isSource && extension === '.html' ? { 'content-security-policy': csp } : {}) }); res.end(bytes); } catch { res.writeHead(404); res.end(); } });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ headless: true, args: ['--force-color-profile=srgb'] }); browserVersion = browser.version();
  for (const input of variants) await check('VCP-capture-' + input.id, async () => {
    let product, design; try {
      product = await candidate(input); const productGeometry = await geometry(product.element), actual = `${input.id}.candidate.png`; await product.element.screenshot({ path: path.join(out, actual), animations: 'disabled' });
      const actualDom = await json(`${input.id}.candidate.dom.json`, { url: product.p.url(), html: await product.element.evaluate(node => node.outerHTML) });
      design = await reference(input); let expected = null, referenceGeometry = null, comparison = { status: 'blocked_missing_source_state', reason: design.blocked };
      if (!design.blocked) { referenceGeometry = await geometry(design.element); const file = `${input.id}.reference.png`; await design.element.screenshot({ path: path.join(out, file), animations: 'disabled' }); expected = await artifact(file); await json(`${input.id}.reference.dom.json`, { url: design.p.url(), html: await design.element.evaluate(node => node.outerHTML), producer: 'unmodified_approved_source_previewHTML' }); comparison = await compare(input.id, expected, await artifact(actual)); }
      const layoutIssues = value => {
        if (!value) return [];
        const [heading, image, meta, actions] = value.children;
        return [image.y + image.height > meta.y && image.x === meta.x ? 'image_overlaps_metadata' : null, meta.scrollHeight > meta.clientHeight ? 'metadata_content_overflows' : null, image.y + image.height > actions.y && image.x === actions.x ? 'image_overlaps_actions' : null].filter(Boolean);
      };
      const actualLayoutIssues = layoutIssues(productGeometry), expectedLayoutIssues = layoutIssues(referenceGeometry);
      const geometryFile = await json(`${input.id}.geometry.json`, { input: { ...input, title: input.title ?? title, clock, frameSha256: frame.sha256, targetFacts: { ...sharedTargetFacts, gameTitle: input.title ?? title, windowTitle: input.title ?? title }, owner: 'candidate', viewport: product.p.viewportSize(), deviceScaleFactor: 1 }, expected: referenceGeometry, actual: productGeometry, expectedLayoutIssues, actualLayoutIssues, classification: actualLayoutIssues.length && expectedLayoutIssues.length ? 'source_and_product_overflow' : actualLayoutIssues.length ? 'product_overflow' : expectedLayoutIssues.length ? 'source_overflow_product_safeguard' : 'no_detected_overlap' });
      comparisons.push({ id: input.id, inputs: input, reference: expected, candidate: await artifact(actual), actualDom, geometry: geometryFile, referenceStatus: design.blocked ? 'blocked_missing_source_state' : 'candidate_pending_independent_review', ...comparison });
      if (!design.blocked && ['narrow-dark-live', 'narrow-light-live', 'narrow-dark-zoom200', 'narrow-light-long-title-zoom200'].includes(input.id)) {
        await product.element.getByRole('button', { name: '预览详情', exact: true }).click();
        await design.element.getByRole('button', { name: '预览详情', exact: true }).click();
        const actualDialog = product.p.getByRole('dialog', { name: '预览详情', exact: true }), expectedDialog = design.p.locator('#rw-preview-details');
        await actualDialog.waitFor(); await expectedDialog.waitFor();
        const modalBounds = await actualDialog.locator('..').boundingBox(), screen = product.p.viewportSize(); assert.ok(modalBounds.x >= 0 && modalBounds.y >= 0 && modalBounds.x + modalBounds.width <= screen.width && modalBounds.y + modalBounds.height <= screen.height, 'Actual details modal fits its real viewport at this zoom');
        const actualFile = `${input.id}-details.candidate.png`, expectedFile = `${input.id}-details.reference.png`;
        await actualDialog.locator('..').screenshot({ path: path.join(out, actualFile), animations: 'disabled' }); await expectedDialog.screenshot({ path: path.join(out, expectedFile), animations: 'disabled' });
        const actualArtifact = await artifact(actualFile), expectedArtifact = await artifact(expectedFile);
        detailComparisons.push({ id: input.id + '-details', inputs: input, reference: expectedArtifact, candidate: actualArtifact, referenceStatus: 'candidate_pending_independent_review', ...await compare(input.id + '-details', expectedArtifact, actualArtifact), geometry: await json(`${input.id}-details.geometry.json`, { expected: await geometry(expectedDialog), actual: await geometry(actualDialog.locator('..')) }) });
        await product.p.keyboard.press('Escape'); await design.p.keyboard.press('Escape');
      }
      assert.deepEqual(actualLayoutIssues, [], 'Actual card must leave metadata and actions unobscured; source overflow is retained as a design conflict');
      if (referenceGeometry) { assert.equal(productGeometry.surface.x, referenceGeometry.surface.x); assert.equal(productGeometry.surface.y, referenceGeometry.surface.y); assert.equal(productGeometry.surface.fontFamily, referenceGeometry.surface.fontFamily); assert.equal(productGeometry.surface.backdropFilter, referenceGeometry.surface.backdropFilter, 'Built product must retain current-source inset material'); }
      assert.equal(productGeometry.theme, input.theme); assert.equal(productGeometry.surface.width, input.width * (input.zoom ?? 1)); assert.equal(productGeometry.surface.height, 346 * (input.zoom ?? 1));
      assert.equal(productGeometry.images.length, ['live', 'paused', 'stale'].includes(input.state) ? 1 : 0, 'Permissions/closed/unavailable never inherit a frame');
      if (productGeometry.images.length) { assert.equal(productGeometry.images[0].naturalWidth, frame.width); assert.equal(productGeometry.images[0].naturalHeight, frame.height); assert.equal(productGeometry.images[0].objectFit, 'contain'); }
      return { geometry: geometryFile, comparisonStatus: comparison.status };
    } finally { for (const item of [product, design]) if (item?.p) { cspViolations.push(...await item.p.evaluate(() => window.__c35CspViolations ?? [])); await item.p.close(); } }
  });
  await check('VCP-details-copy-escape-focus-without-capture-churn', async () => {
    const { p, element } = await candidate(variants[0]); try {
      const before = await ownership(p); await p.context().grantPermissions(['clipboard-read', 'clipboard-write']); await p.evaluate(() => navigator.clipboard.writeText('c35-before-copy'));
      const trigger = element.getByRole('button', { name: '预览详情', exact: true }); await trigger.click(); const dialog = p.getByRole('dialog', { name: '预览详情', exact: true }); await dialog.waitFor();
      assert.equal(await trigger.getAttribute('aria-expanded'), 'true'); assert.equal(await dialog.getByText(title, { exact: true }).count() > 0, true);
      const scrollRegion = dialog.getByRole('region', { name: '可滚动的完整预览详情', exact: true }); await scrollRegion.focus(); const fixed = { header: await dialog.locator('header').boundingBox(), footer: await dialog.locator('footer').boundingBox() }; await p.keyboard.press('End'); await p.waitForFunction(() => document.querySelector('[aria-label="可滚动的完整预览详情"]').scrollTop > 0); assert.deepEqual({ header: await dialog.locator('header').boundingBox(), footer: await dialog.locator('footer').boundingBox() }, fixed); await p.keyboard.press('Home'); await p.waitForFunction(() => document.querySelector('[aria-label="可滚动的完整预览详情"]').scrollTop === 0);
      const detailFile = 'details-dark.candidate.png'; await dialog.locator('..').screenshot({ path: path.join(out, detailFile), animations: 'disabled' });
      await dialog.getByRole('button', { name: '复制全部', exact: true }).click(); const copied = [await p.evaluate(() => navigator.clipboard.readText())]; assert.notEqual(copied[0], 'c35-before-copy'); assert.ok(copied[0].includes(title)); assert.ok(copied[0].includes('current-c35-candidate')); assert.equal(copied[0].includes(frame.base64.slice(0, 80)), false); assert.equal(/"payload"\s*:/.test(copied[0]), false);
      for (const key of ['Tab', 'Tab', 'Shift+Tab']) { await p.keyboard.press(key); assert.equal(await dialog.evaluate(node => node.contains(document.activeElement)), true, 'Keyboard focus stays in the actual modal'); }
      await p.evaluate(() => { window.__c35OriginalClipboardWrite = navigator.clipboard.writeText.bind(navigator.clipboard); Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: async () => { throw Error('Explicit controlled clipboard denial'); } }); });
      await dialog.getByRole('button', { name: '复制全部', exact: true }).click(); await dialog.getByText('复制失败，可在详情中选择文字', { exact: true }).waitFor(); assert.equal(await dialog.locator('dd').first().evaluate(node => getComputedStyle(node).userSelect), 'text');
      await p.evaluate(() => { Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: window.__c35OriginalClipboardWrite }); });
      await p.keyboard.press('Escape'); await dialog.waitFor({ state: 'hidden' }); await p.waitForFunction(node => node === document.activeElement, await trigger.elementHandle()); assert.equal(await trigger.evaluate(node => node === document.activeElement), true); assert.deepEqual(await ownership(p), before);
      await trigger.click(); await dialog.waitFor();
      await p.evaluate(() => { window.__c35HeldCopies = []; Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: text => { window.__c35HeldCopies.push(text); return new Promise(resolve => { window.__c35ReleaseCopy = resolve; }); } }); });
      const copyButton = dialog.getByRole('button', { name: '复制全部', exact: true }); await copyButton.click(); await copyButton.click(); await json('held-copy-observation.json', await copyButton.evaluate(node => ({ button: node.outerHTML, copiedRequests: window.__c35HeldCopies.length, activeElement: document.activeElement?.outerHTML }))); await p.waitForFunction(() => [...document.querySelectorAll('[role=dialog] button')].find(button => button.textContent.includes('复制全部'))?.getAttribute('aria-busy') === 'true'); assert.equal(await copyButton.getAttribute('aria-busy'), 'true'); assert.equal(await p.evaluate(() => window.__c35HeldCopies.length), 1, 'Synchronous pending guard prevents duplicate clipboard requests while keeping the button focusable');
      await p.keyboard.press('Escape'); await dialog.waitFor({ state: 'hidden' }); await trigger.click(); await dialog.waitFor(); await p.evaluate(() => window.__c35ReleaseCopy()); await p.waitForFunction(() => [...document.querySelectorAll('[role="dialog"] button')].find(button => button.textContent.includes('复制全部'))?.getAttribute('aria-busy') !== 'true'); assert.equal(await dialog.getByText('已复制完整详情', { exact: true }).count(), 0, 'A prior open-cycle completion cannot label the reopened details copied');
      await p.evaluate(() => { Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: window.__c35OriginalClipboardWrite }); });
      await p.evaluate(() => window.__setReviewedPreviewInputs({ title: '更新后的游戏窗口标题', detail: 'Updated Bottle / full path', identityDetail: 'updated-window · PID 7000' }));
      await element.getByText('更新后的游戏窗口标题', { exact: true }).waitFor(); assert.equal(await dialog.getByText('更新后的游戏窗口标题', { exact: true }).count() > 0, true);
      await p.evaluate(() => { const port = window.uiHarness, previous = [...port.previewProjections.values()].find(value => value.consumerId === 'current-c35-candidate'); port.emitPreview({ projection: { ...previous, state: 'stale', detail: 'Updated controlled stale diagnostic' }, frame: null }); });
      await p.waitForFunction(() => document.querySelector('[data-preview-state]')?.dataset.previewState === 'stale'); await dialog.getByText('Updated controlled stale diagnostic', { exact: true }).waitFor(); assert.deepEqual(await ownership(p), before);
      await p.keyboard.press('Escape'); return { before, after: await ownership(p), copiedTextSha256: sha(copied[0]), screenshot: await artifact(detailFile), actions: ['open', 'keyboard_scroll_independent_body', 'actual_browser_clipboard_copy', 'focus_trap', 'explicit_clipboard_denial', 'manual_text_fallback', 'Escape', 'focus_return', 'held_copy_busy_and_no_double_dispatch', 'close_reopen_ignores_old_copy_completion', 'reopen', 'title_context_update', 'stale_update', 'Escape'] };
    } finally { await p.close(); }
  });
  await check('VCP-candidate-session-two-consumers-are-isolated', async () => {
    const p = await pageBase(variants[0], 'production_two_owners');
    try {
      await p.addInitScript({ content: `window.__reviewedPreviewInputs = ${JSON.stringify({ title, width: 282 })}; window.__initializeControlledPort = port => { (${initialPort.toString()})(port, ${JSON.stringify({ ...variants[0], clock, frame })}); const candidate = port.reviewedCandidatePreviewFixture; const sessionId = 'current-c35-session'; port.state.session = { ...(port.state.session ?? {}), sessionId, gameId: 'ui-game', sourceKind: 'hook', state: 'active', paused: false, translationEnabled: true, sources: [] }; const sessionProjection = port.prepareSessionPreview(sessionId, 'b'.repeat(64), 'starting'); port.reviewedPreviewPairFixtures = { candidate, session: { candidateId: '', projection: sessionProjection } }; };` });
      await p.goto(origin + '/tests/visual/index.html?role=main&scenario=empty&reviewedSurface=preview-pair');
      const candidate = p.getByTestId('preview-pair-candidate').getByRole('region', { name: '运行窗口预览', exact: true }), session = p.getByTestId('preview-pair-session').getByRole('region', { name: '运行窗口预览', exact: true });
      await candidate.locator('img').waitFor(); await p.waitForFunction(() => window.uiHarness.previewListeners.size === 2);
      await p.evaluate(({ frame, clock }) => { const port = window.uiHarness, projection = [...port.previewProjections.values()].find(value => value.consumerId === 'current-c35-session'); port.emitPreview({ projection: { ...projection, state: 'live', capturedAtMs: String(clock) }, frame: { leaseId: projection.leaseId, generation: projection.generation, sequence: '1', targetIdentityHash: projection.targetIdentityHash, capturedAtMs: String(clock), width: frame.width, height: frame.height, contentHash: frame.sha256, encoding: 'jpeg', payload: [...Uint8Array.from(atob(frame.base64), c => c.charCodeAt(0))] } }); }, { frame, clock });
      await session.locator('img').waitFor(); const sessionImage = await session.locator('img').getAttribute('src'), before = await ownership(p);
      assert.equal(before.active.length, 2); assert.equal(new Set(before.active.map(owner => owner.consumerId)).size, 2);
      await candidate.getByRole('button', { name: '预览详情', exact: true }).click(); const dialog = p.getByRole('dialog', { name: '预览详情', exact: true }); await dialog.waitFor(); await dialog.getByText('current-c35-candidate', { exact: true }).waitFor(); assert.equal(await dialog.getByText('current-c35-session', { exact: true }).count(), 0); await p.keyboard.press('Escape'); assert.deepEqual(await ownership(p), before);
      await p.evaluate(() => { const port = window.uiHarness, previous = [...port.previewProjections.values()].find(value => value.consumerId === 'current-c35-candidate'); port.emitPreview({ projection: { ...previous, state: 'target_closed', detail: 'Only candidate closed' }, frame: null }); });
      await p.waitForFunction(() => document.querySelector('[data-testid="preview-pair-candidate"] [data-preview-state]')?.dataset.previewState === 'target_closed');
      assert.equal(await candidate.locator('img').count(), 0); assert.equal(await session.getAttribute('data-preview-state'), 'live'); assert.equal(await session.locator('img').getAttribute('src'), sessionImage);
      await session.getByRole('button', { name: '预览详情', exact: true }).click(); await dialog.waitFor(); await dialog.getByText('current-c35-session', { exact: true }).waitFor(); assert.equal(await dialog.getByText('current-c35-candidate', { exact: true }).count(), 0); await p.keyboard.press('Escape'); assert.deepEqual(await ownership(p), before);
      const file = 'two-consumers.candidate.png'; await p.screenshot({ path: path.join(out, file), animations: 'disabled' }); return { before, after: await ownership(p), screenshot: await artifact(file), actions: ['candidate_details', 'candidate_closed', 'session_live_preserved', 'session_details'] };
    } finally { await p.close(); }
  });
} catch (error) { checks.push({ id: 'VCP-runner', status: 'failed', error: error.message, stack: error.stack }); }
finally { await browser?.close(); if (server) await new Promise(resolve => server.close(resolve)); }
const sourceDrift = []; for (const [file, expected] of Object.entries(sourceSha256)) if (sha(await fs.readFile(path.join(root, file))) !== expected) sourceDrift.push(file);
for (const [file, expected] of Object.entries(approval.design_sources)) if (sha(await fs.readFile(path.join(root, file))) !== expected) sourceDrift.push(file);
const behaviorStatus = checks.length && checks.every(row => row.status === 'passed') && !sourceDrift.length && !pageErrors.length && !requestViolations.length && !cspViolations.length ? 'passed' : 'failed';
const pixelStatus = comparisons.length === variants.length && detailComparisons.length === 4 && [...comparisons, ...detailComparisons].every(row => row.status === 'passed') ? 'passed' : 'failed';
const report = { schemaVersion: 1, kind: 'current-preview-visuals/v1', executionNonce: nonce, startedAt, finishedAt: new Date().toISOString(), status: behaviorStatus === 'passed' && pixelStatus === 'passed' ? 'passed' : 'failed', behaviorStatus, pixelStatus, referenceApproval: 'pending_independent_review', productAcceptance: 'blocked', packagedNativeVerified: false, allStateAdapterRegistered: false, assertionCount: assertions, skippedAssertions: 0, command: [process.execPath, ...process.argv.slice(1)], sourceSha256, sourceInputs: await artifact('source-inputs.json'), sourceDrift, sharedTargetFacts, approvedDesign: { approvalFile, approvalSha256: sha(approvalBytes), compositionId: approval.composition_id, closureSha256: approval.design_closure_sha256, sourceCount: 61, sources: approval.design_sources }, historicalInventory, sharedFrame: { ...frameInput, artifact: await artifact('shared-frame.jpg') }, builds, sourceMaps: maps, environment: { node: process.version, platform: process.platform, arch: process.arch }, browser: { version: browserVersion, executablePath: chromium.executablePath(), playwright: require('playwright-core/package.json').version, engine: 'Playwright-managed pinned headless Chromium', deviceScaleFactor: 1, locale: 'zh-CN', timezone: 'Asia/Shanghai' }, productionCsp: csp, checks, comparisons, detailComparisons, pageErrors, requestViolations, cspViolations, limitations: ['Current-source DOM screenshots are independent reference candidates; no approval is inferred.', 'Approved source has offline replay labels, frozen time formatting, and no stale mode; real differences/blocked state remain.', 'Finite browser/controlled ApplicationPort checks cannot satisfy native capture, package, full794 obligations or new details-state references.', 'Historical v4 37 states, original PNGs, zero-ratio threshold and five retired-precondition blocks remain unchanged.'] };
await json('verification.json', report);
const escape = value => String(value).replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]));
await fs.writeFile(path.join(out, 'index.html'), `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>C35当前设计与真实React</title><style>body{font:14px/1.6 -apple-system,sans-serif;background:#20232c;color:#eee;padding:24px}section{margin:30px 0}figure{display:inline-block;width:30%;vertical-align:top;margin:1%}img{max-width:100%}pre{white-space:pre-wrap}</style><h1>C35当前设计与真实React</h1><p>行为 ${behaviorStatus}；零阈值像素 ${pixelStatus}；独立参照候选待审核，产品与Native仍blocked。</p>${checks.map(row => `<p>${escape(row.id)}: ${escape(row.status)} ${escape(row.error ?? '')}</p>`).join('')}${[...comparisons, ...detailComparisons].map(row => `<section><h2>${escape(row.id)} · ${escape(row.status)}</h2><p>${escape(row.reason ?? '')} ${row.pixels ?? ''} 像素</p>${[['reference', '批准源真实DOM参照候选'], ['candidate', '真实React候选'], ['diff', '原PNG差异']].map(([key, label]) => row[key] ? `<figure><img src="${row[key].path}"><figcaption>${label}</figcaption></figure>` : '').join('')}</section>`).join('')}</html>`);
console.log(JSON.stringify({ status: report.status, behaviorStatus, pixelStatus, checks: checks.length, comparisons: comparisons.length, report: path.relative(root, path.join(out, 'verification.json')) }));
process.exitCode = report.status === 'passed' ? 0 : 1;
