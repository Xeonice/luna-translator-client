import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash, randomUUID } from 'node:crypto';
import nodeAssert from 'node:assert/strict';
let assertionCount = 0;
const assert = new Proxy(nodeAssert, { apply(target, thisArg, args) { assertionCount += 1; return Reflect.apply(target, thisArg, args); }, get(target, key) { const value = Reflect.get(target, key); return typeof value === 'function' ? (...args) => { assertionCount += 1; return Reflect.apply(value, target, args); } : value; } });
const here = path.dirname(fileURLToPath(import.meta.url)), root = path.resolve(here, '../../../..'), desktop = path.join(root, 'apps/desktop');
const require = createRequire(path.join(root, 'package.json')), { chromium } = require('playwright-core');
const hash = data => createHash('sha256').update(data).digest('hex');
// SR2 actual-module test loader: only external Tauri is stubbed.
function loadActualTauriPort(stubs) {
  const bridge = path.join(desktop, 'src/bridge');
  const ts = require(path.join(desktop, 'node_modules/typescript'));
  const external = new Set(['@tauri-apps/api/event', '@tauri-apps/api/core', '@tauri-apps/api/window', '@tauri-apps/api/webview']);
  const cache = new Map();
  const load = file => {
    const relative = path.relative(bridge, file);
    assert.equal(path.isAbsolute(relative) || relative.startsWith('..' + path.sep) || relative === '..', false, 'Actual adapter dependencies must stay in the production bridge');
    assert.equal(path.extname(file), '.ts');
    assert.equal(fsSync.realpathSync(file), file, 'Actual adapter dependencies must not traverse symlinks');
    assert.equal(fsSync.lstatSync(file).isFile(), true);
    if (cache.has(file)) return cache.get(file).exports;
    const module = { exports: {} }; cache.set(file, module);
    const compiled = ts.transpileModule(fsSync.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
    new Function('require', 'exports', 'module', compiled)(name => {
      if (external.has(name)) { assert.ok(Object.hasOwn(stubs, name)); return stubs[name]; }
      assert.equal(name.startsWith('./') || name.startsWith('../'), true, 'Only the declared external Tauri modules may be stubbed');
      return load(path.resolve(path.dirname(file), name + '.ts'));
    }, module.exports, module);
    return module.exports;
  };
  return new (load(path.join(bridge, 'tauri-port.ts')).TauriApplicationPort)();
}

// SR2 regression vectors execute the actual production adapter, including its private dependencies.
async function assertUnsettledPreviewContext(actualAdapter, waitUntil) {
  for (const rejectOld of [false, true]) {
    const calls = [], old = [], received = [], pending = new Map(), maximum = new Map();
    const projection = generation => ({ leaseId: 'controlled-never-settled-lease', generation, consumerId: 'controlled-context-owner', state: 'live', permission: 'granted', targetIdentityHash: 'a'.repeat(64), attemptedAtMs: null, capturedAtMs: '101', contentChangedAtMs: '101', permissionCheckedAtMs: null, errorCode: null, detail: 'Explicit controlled context' });
    const update = (generation, sequence) => ({ projection: projection(generation), frame: { leaseId: 'controlled-never-settled-lease', generation, sequence: String(sequence), targetIdentityHash: 'a'.repeat(64), capturedAtMs: String(101 + sequence), width: 160, height: 100, contentHash: 'b'.repeat(64), encoding: 'jpeg', payload: [1, 2, 3] } });
    let releaseOld, releaseCurrent, currentReads = 0;
    const adapter = await actualAdapter(async (name, args) => {
      assert.equal(name, 'application_preview_latest'); calls.push({ name, args });
      const count = (pending.get(args.generation) ?? 0) + 1; pending.set(args.generation, count); maximum.set(args.generation, Math.max(maximum.get(args.generation) ?? 0, count));
      try {
        if (args.generation === '1') return await new Promise((resolve, reject) => { releaseOld = () => rejectOld ? reject(Error('Controlled late old rejection')) : resolve(update('1', 100)); });
        currentReads += 1;
        if (currentReads === 2) await new Promise(resolve => { releaseCurrent = resolve; });
        return update('2', currentReads);
      } finally { pending.set(args.generation, pending.get(args.generation) - 1); }
    });
    const offOld = adapter.attachWindowPreview(projection('1'), value => old.push(value)); let offCurrent = () => {};
    try {
      await waitUntil(() => !!releaseOld); offOld(); offCurrent = adapter.attachWindowPreview(projection('2'), value => received.push(value));
      await waitUntil(() => received.length >= 1);
      assert.equal(pending.get('1'), 1, 'New context must progress while the old invocation is still unsettled');
      assert.equal(old.length, 0); assert.ok(received.every(value => value.projection.generation === '2'));
      await waitUntil(() => !!releaseCurrent); const callsBeforeLateOld = currentReads;
      releaseOld(); await new Promise(resolve => setTimeout(resolve, 20));
      assert.equal(currentReads, callsBeforeLateOld, 'Old finally cannot schedule or clear the current pending read');
      assert.equal(pending.get('2'), 1); releaseCurrent(); await waitUntil(() => received.length >= 2);
      assert.equal(maximum.get('1'), 1); assert.equal(maximum.get('2'), 1); assert.equal(old.length, 0);
      assert.equal(calls.filter(value => value.args.generation === '2')[0].args.afterSequence, null);
    } finally { offOld(); offCurrent(); releaseOld?.(); releaseCurrent?.(); }
  }
}
async function assertMetadataOnlyPreviewRecovery(actualAdapter, waitUntil) {
  const projection = { leaseId: 'controlled-metadata-only-lease', generation: '1', consumerId: 'controlled-metadata-owner', state: 'live', permission: 'granted', targetIdentityHash: 'a'.repeat(64), attemptedAtMs: null, capturedAtMs: '101', contentChangedAtMs: '101', permissionCheckedAtMs: null, errorCode: null, detail: 'Explicit controlled metadata' };
  const original = { leaseId: projection.leaseId, generation: '1', sequence: '1', targetIdentityHash: projection.targetIdentityHash, capturedAtMs: '101', width: 160, height: 100, contentHash: 'b'.repeat(64), encoding: 'jpeg', payload: [1, 2, 3] };
  const first = [], joining = [], calls = []; let mode = 'frame', metadataReads = 0;
  const adapter = await actualAdapter(async (name, args) => {
    assert.equal(name, 'application_preview_latest'); calls.push({ name, args });
    if (mode === 'failure') throw { code: 'preview_transport', stage: 'capture', message: 'Controlled local read failure', recoverable: true };
    if (mode === 'metadata') { metadataReads += 1; return { projection: { ...projection, capturedAtMs: '999' }, frame: null }; }
    return { projection: { ...projection, capturedAtMs: mode === 'fresh' ? '202' : '101' }, frame: mode === 'fresh' ? { ...original, sequence: '2', capturedAtMs: '202' } : original };
  });
  const offFirst = adapter.attachWindowPreview(projection, value => first.push(value)); let offJoining = () => {};
  try {
    await waitUntil(() => first.some(value => value.frame)); mode = 'failure';
    await waitUntil(() => first.some(value => value.projection.errorCode === 'preview_transport')); mode = 'metadata';
    await waitUntil(() => metadataReads >= 1); const beforeJoin = first.length;
    offJoining = adapter.attachWindowPreview(projection, value => joining.push(value)); await waitUntil(() => joining.length >= 1);
    assert.equal(first.at(-1).projection.state, 'stale', 'A metadata-only success cannot clear the local transport notice');
    assert.equal(joining[0].projection.state, 'stale', 'A new listener must not replay an old JPEG as Live after metadata-only recovery');
    assert.equal(joining[0].projection.errorCode, 'preview_transport'); assert.deepEqual(joining[0].frame, original);
    assert.equal(joining[0].frame.capturedAtMs, '101'); assert.ok(first.length >= beforeJoin);
    mode = 'fresh'; await waitUntil(() => first.some(value => value.frame?.sequence === '2') && joining.some(value => value.frame?.sequence === '2'));
    assert.equal(first.at(-1).projection.state, 'live'); assert.equal(joining.at(-1).projection.state, 'live');
    assert.equal(first.at(-1).projection.errorCode, null); assert.equal(joining.at(-1).frame.capturedAtMs, '202');
    assert.ok(calls.every(value => value.name === 'application_preview_latest'));
  } finally { offFirst(); offJoining(); }
}
// End SR2 actual adapter regression helpers.

const securityConfigFile = 'apps/desktop/src-tauri/tauri.conf.json';
const securityConfigBytes = await fs.readFile(path.join(root, securityConfigFile));
const productionCsp = JSON.parse(securityConfigBytes).app.security.csp;
assert.equal(typeof productionCsp, 'string'); assert.ok(productionCsp.length > 0);
const productionSecurityPolicy = { sourceFile: securityConfigFile, sourceSha256: hash(securityConfigBytes), csp: productionCsp, delivery: 'document_response_header', violations: [] };
const outputIndex = process.argv.indexOf('--output'), nonceIndex = process.argv.indexOf('--nonce');
const nonce = nonceIndex >= 0 ? process.argv[nonceIndex + 1] : randomUUID();
if (!nonce) throw new Error('An execution nonce is required');
const fixtureIndex = process.argv.indexOf('--current-session-fixtures');
if (fixtureIndex < 0 || !process.argv[fixtureIndex + 1]) throw new Error('--current-session-fixtures <fresh SQLite manifest> is required; the four authoritative projection scenarios cannot be skipped or substituted.');
const manifestPath = path.resolve(root, process.argv[fixtureIndex + 1]);
const manifestBytes = await fs.readFile(manifestPath), fixtureManifest = JSON.parse(manifestBytes);
assert.equal(fixtureManifest.schemaVersion, 1); assert.equal(fixtureManifest.kind, 'actual_sqlite_runtime_projection_fixtures');
assert.equal(fixtureManifest.producerTarget, 'application_runtime'); assert.equal(fixtureManifest.producer, 'exports_current_session_component_snapshots'); assert.equal(fixtureManifest.nonce, nonce);
assert.equal(fixtureManifest.notLiveGameEvidence, true);
const fixtureSourceFiles = { 'crates/app-infra/src/storage/queries/mod.rs': fixtureManifest.snapshotQuerySourceSha256, 'crates/app-infra/tests/application_runtime.rs': fixtureManifest.producerSourceSha256 };
for (const [file, expected] of Object.entries(fixtureSourceFiles)) assert.equal(hash(await fs.readFile(path.join(root, file))), expected, `SQLite fixture producer source changed: ${file}`);
const fixtureScenarios = ['01-new-session-waiting', '02-old-session-late', '03-new-session-body', '04-disconnected-last-sentence'];
assert.deepEqual(fixtureManifest.entries.map(entry => entry.scenario).sort(), [...fixtureScenarios].sort());
const sqlProjectionFixtures = new Map(), fixtureInputs = [];
for (const entry of fixtureManifest.entries) {
  assert.equal(path.basename(entry.path), entry.path, 'SQLite scenario path must be a local filename');
  const file = path.join(path.dirname(manifestPath), entry.path), stat = await fs.lstat(file);
  assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, 'SQLite scenario must be a regular generated file');
  const bytes = await fs.readFile(file); assert.equal(hash(bytes), entry.sha256, `SQLite scenario identity changed: ${entry.scenario}`);
  const snapshot = JSON.parse(bytes); assert.equal(snapshot.session?.sessionId, entry.sessionId);
  if (entry.current === null) assert.equal(snapshot.current, null);
  else for (const [key, value] of Object.entries(entry.current)) assert.deepEqual(snapshot.current?.[key], value);
  sqlProjectionFixtures.set(entry.scenario, snapshot); fixtureInputs.push({ scenario: entry.scenario, path: entry.path, fileRootRelative: path.relative(root, file), sha256: entry.sha256 });
}
const currentSessionFixtures = { manifestFile: path.relative(root, manifestPath), fixtureManifestSha256: hash(manifestBytes), nonce, producerTarget: fixtureManifest.producerTarget, producer: fixtureManifest.producer, snapshotQuerySourceSha256: fixtureManifest.snapshotQuerySourceSha256, producerSourceSha256: fixtureManifest.producerSourceSha256, sourceSha256: fixtureSourceFiles, files: fixtureInputs };
assert.ok(fixtureManifest.readingStatusHistory, 'The actual SQLite history/status producer is required');
const readingHistoryEntry = fixtureManifest.readingStatusHistory;
assert.equal(path.basename(readingHistoryEntry.path), readingHistoryEntry.path);
const readingHistoryFile = path.join(path.dirname(manifestPath), readingHistoryEntry.path);
const readingHistoryStat = await fs.lstat(readingHistoryFile);
assert.equal(readingHistoryStat.isFile() && !readingHistoryStat.isSymbolicLink(), true);
const readingHistoryBytes = await fs.readFile(readingHistoryFile);
assert.equal(hash(readingHistoryBytes), readingHistoryEntry.sha256);
const actualReadingHistory = JSON.parse(readingHistoryBytes);
assert.equal(actualReadingHistory.schemaVersion, 1);
assert.equal(actualReadingHistory.kind, 'actual_sqlite_reading_unit_projection_history');
assert.equal(actualReadingHistory.nonce, nonce);
assert.equal(typeof actualReadingHistory.sourceSha256, 'object');
assert.ok(actualReadingHistory.sourceSha256 && !Array.isArray(actualReadingHistory.sourceSha256));
for (const [file, expected] of Object.entries(actualReadingHistory.sourceSha256)) {
  assert.ok(Object.hasOwn(sourcePathAllowlist(), file), `Undeclared history producer source: ${file}`);
  assert.equal(hash(await fs.readFile(path.join(root, file))), expected, `History producer source changed: ${file}`);
}
for (const file of Object.keys(fixtureSourceFiles)) assert.equal(actualReadingHistory.sourceSha256[file], fixtureSourceFiles[file]);
function sourcePathAllowlist() { return { ...fixtureSourceFiles, 'crates/app-infra/src/storage/queries/translation_status.rs': true, 'crates/app-core/src/domain/translation_status.rs': true, 'crates/app-infra/src/storage/queries/tasks.rs': true, 'crates/app-infra/src/storage/database.rs': true, 'crates/app-core/src/application/runtime.rs': true, 'crates/app-core/src/contracts/desktop.rs': true }; }
currentSessionFixtures.readingStatusHistory = { path: readingHistoryEntry.path, fileRootRelative: path.relative(root, readingHistoryFile), sha256: readingHistoryEntry.sha256, sourceSha256: actualReadingHistory.sourceSha256 };

async function filesUnder(directory) { const result=[];for(const entry of await fs.readdir(directory,{withFileTypes:true})){const file=path.join(directory,entry.name);if(entry.isDirectory())result.push(...await filesUnder(file));else if(entry.isFile())result.push(file);}return result;}
const auditedFiles=[...await filesUnder(path.join(desktop,'src')),...await filesUnder(path.join(desktop,'tests'))];
const sourceBefore=Object.fromEntries(await Promise.all(auditedFiles.sort().map(async file=>[path.relative(root,file),hash(await fs.readFile(file))])));
const out = outputIndex >= 0 ? path.resolve(process.argv[outputIndex + 1]) : path.join(root, '.build/next-batch-20261020/ui', `components-${new Date().toISOString().replace(/[:.]/g, '-')}`);
if (fsSync.existsSync(out)) throw new Error(`Refusing to overwrite evidence: ${out}`);
await fs.mkdir(out, { recursive: true });
const rows = [], captures = [], errors = [], additionalTestBuilds = [], additionalChildren = []; let browser, child, strictChild;
async function check(id, execute) { const before=assertionCount; try { await execute(); rows.push({ id, status: 'passed', assertionCount:assertionCount-before }); console.log(`${id}: passed`); } catch (error) { rows.push({ id, status: 'failed', assertionCount:assertionCount-before, error: error.message, stack: error.stack }); console.log(`${id}: failed: ${error.stack ?? error.message}`); } }
// The compact surface exposes raw facts through its real accessible details dialog.
// These helpers retain the historical failure/time assertions while following that UI.
async function previewDiagnosticText(page, preview) {
  const trigger = preview.getByRole('button', { name: '预览详情', exact: true });
  await trigger.click();
  const dialog = page.getByRole('dialog', { name: '预览详情', exact: true });
  await dialog.waitFor();
  const text = await dialog.textContent();
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'hidden' });
  return text;
}
async function previewTimeText(preview) {
  const time = preview.locator('time');
  const value = await time.getAttribute('datetime');
  if (!value) return `画面时间 ${await time.textContent()}`;
  const parsed = new Date(value);
  assert.equal(Number.isFinite(parsed.getTime()), true, 'Displayed JPEG has a genuine exact timestamp');
  return `画面时间 ${parsed.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false, fractionalSecondDigits: 3 })}`;
}
async function screenshot(page, name) { const file = `${name}.candidate.png`; await page.screenshot({ path: path.join(out, file) }); captures.push({ file, sha256: hash(await fs.readFile(path.join(out, file))), scope: 'real_react_controlled_data_not_native_or_game', referenceStatus: 'candidate_not_approved_or_pixel_pass' }); }
// The reviewed CP-01 retires mutually exclusive tabs. Older focused cases
// explicitly open one disclosure; simultaneous-open retention has its own journey.
async function openWindowManagement(page) {
  const summary = page.locator('summary').filter({ hasText: /^窗口关联管理$/ });
  const owner = summary.locator('..');
  if (await owner.getAttribute('open') === null) await summary.click();
}

async function openSessionDataset(page, name, { exclusive = true } = {}) {
  const identity = { '正文历史': 'reading', '全部提取': 'texts', 'Hook 日志': 'logs' }[name];
  assert.ok(identity, `Unknown session dataset: ${name}`);
  if (exclusive) for (const other of ['reading', 'texts', 'logs']) {
    const node = page.locator(`details[data-flat-session-section="${other}"]`);
    if (other !== identity && await node.getAttribute('open') !== null) await node.locator(':scope > summary').click();
  }
  const node = page.locator(`details[data-flat-session-section="${identity}"]`);
  if (await node.getAttribute('open') === null) await node.locator(':scope > summary').click();
  await node.locator('[data-session-dataset]').waitFor({ state: 'visible' });
  return node;
}

async function waitSessionSummaryCount(page, section, count) {
  const summary = page.locator(`details[data-flat-session-section="${section}"] > summary`);
  await summary.filter({ hasText: new RegExp(`上次读取 · ${count} 条`) }).waitFor();
  assert.equal(await summary.isVisible(), true, 'The retained range count is displayed in the disclosure summary');
}

async function navigateWithProductionCsp(page, url) {
  page.on('console', message => { if (message.type() === 'error' && message.text().includes('Content Security Policy')) productionSecurityPolicy.violations.push({ url: page.url(), message: message.text() }); });
  await page.route('**/*', async route => {
    if (route.request().resourceType() !== 'document') return route.continue();
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': productionCsp } });
  });
  const response = await page.goto(url);
  assert.equal(await response.headerValue('content-security-policy'), productionCsp, 'Actual document response must enforce the unchanged production CSP');
}
try {
  const build = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(here, 'vite.config.ts'), '--outDir', path.join(out, 'controlled-build')], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let buildLog = ''; build.stdout.on('data', data => { buildLog += data; }); build.stderr.on('data', data => { buildLog += data; }); const buildCode = await new Promise(resolve => build.on('exit', resolve)); await fs.writeFile(path.join(out, 'build.log'), buildLog); assert.equal(buildCode, 0, buildLog);
  const listener = createServer(); await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve)); const port = listener.address().port; await new Promise(resolve => listener.close(resolve));
  child = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--outDir', path.join(out, 'controlled-build'), '--host', '127.0.0.1', '--port', String(port), '--strictPort'], { cwd: desktop, stdio: 'ignore' });
  const url = `http://127.0.0.1:${port}/tests/visual/index.html?window=main&probe=reading-controls`;
  for (let attempt = 0; attempt < 100; attempt++) { try { if ((await fetch(url)).ok) break; } catch {} await new Promise(resolve => setTimeout(resolve, 100)); }
  browser = await chromium.launch({ headless: true, ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE } : {}) });
  const page = await browser.newPage({ viewport: { width: 800, height: 900 }, deviceScaleFactor: 1 }); page.on('pageerror', error => errors.push(error.message)); await navigateWithProductionCsp(page, url); await page.getByTestId('reviewed-reading-probe').waitFor();
  const trigger = page.getByRole('combobox', { name: '示例文本来源', exact: true });
  await check('RBC-01-custom-select-not-visible-native', async () => { assert.equal(await page.locator('select').evaluateAll(nodes => nodes.every(node => node.tabIndex === -1 && node.closest('[aria-hidden="true"]') && getComputedStyle(node.closest('[aria-hidden="true"]')).clip === 'rect(0px, 0px, 0px, 0px)')), true); assert.equal(await trigger.evaluate(node => node.tagName), 'BUTTON'); await trigger.click(); assert.equal(await page.getByRole('listbox').count(), 1); assert.equal(await page.getByRole('option').count(), 44); await page.keyboard.press('Escape'); assert.equal(await trigger.getAttribute('data-select-value'), 'body'); });
  await check('RBC-02-active-single-gap-no-row-shrink', async () => { await trigger.click(); const options = page.getByRole('option'); await options.nth(1).hover(); assert.equal(await page.locator('[role=option][data-focused]').count(), 1); assert.equal(await page.locator('[role=option][data-focused]').getAttribute('data-select-value'), 'interface'); await page.keyboard.press('ArrowDown'); assert.equal(await page.locator('[role=option][data-focused]').count(), 1); assert.equal(await page.locator('[role=option][data-focused]').getAttribute('data-select-value'), 'long'); await options.nth(0).hover(); assert.equal(await page.locator('[role=option][data-focused]').count(), 1); assert.equal(await page.locator('[role=option][data-focused]').getAttribute('data-select-value'), 'body'); const geometry = await options.evaluateAll(nodes => nodes.map(node => { const b = node.getBoundingClientRect(); return { y: b.y, h: b.height, shrink: getComputedStyle(node).flexShrink }; })); for (let i = 1; i < geometry.length; i++) { assert.equal(geometry[i].y - geometry[i - 1].y - geometry[i - 1].h, 2); assert.equal(geometry[i].shrink, '0'); assert.ok(geometry[i].h >= 30); } await screenshot(page, 'select-expanded-single-active'); await page.keyboard.press('Escape'); });
  await check('RBC-03-cancel-outside-and-long-confirm', async () => { await trigger.click(); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Escape'); assert.equal(await trigger.getAttribute('data-select-value'), 'body'); await page.waitForFunction(id => document.activeElement?.id === id, await trigger.getAttribute('id')); assert.equal(await trigger.evaluate(node => node === document.activeElement), true); await trigger.click(); await page.getByTestId('confirmed-select-value').click({ force: true }); assert.equal(await trigger.getAttribute('data-select-value'), 'body'); await trigger.click(); await page.getByRole('option', { name: 'GameN7_Steam.exe · Steam · MAGES2 · 长文本来源名称完整显示', exact: true }).click(); assert.equal(await trigger.getAttribute('data-select-value'), 'long'); assert.match(await trigger.getAttribute('title'), /名称完整显示/); });
  await check('RBC-04-disabled-empty-loading-error-recovery', async () => { assert.equal(await page.getByRole('combobox', { name: '不可用模型', exact: true }).isDisabled(), true); assert.equal(await page.getByRole('combobox', { name: '正在获取模型', exact: true }).isDisabled(), true); const failed = page.getByRole('combobox', { name: '获取失败的模型', exact: true }); assert.equal(await failed.getAttribute('aria-invalid'), 'true'); await page.getByRole('button', { name: '重试', exact: true }).click(); assert.equal(await failed.isEnabled(), true); assert.equal(await failed.getAttribute('aria-invalid'), null); const empty = page.getByRole('combobox', { name: '空模型列表', exact: true }); await empty.click(); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter'); await page.keyboard.press('Escape'); assert.equal(await empty.getAttribute('data-select-value'), ''); });
  const source = page.getByTestId('source-text');
  const original = '放課後、図書館で彼女と待ち合わせをした。';
  await check('RBC-05-eight-independent-annotation-combinations', async () => { for (const segmentation of [false, true]) for (const ruby of [false, true]) for (const pos of [false, true]) { for (const [name, value] of Object.entries({ segmentation, ruby, pos })) await page.getByRole('checkbox', { name, exact: true }).setChecked(value); const actual = await source.evaluate(node => { const clone = node.cloneNode(true); clone.querySelectorAll('rt').forEach(n => n.remove()); return clone.textContent; }); assert.equal(actual, original); assert.equal(await source.locator('rt').count() > 0, ruby); assert.equal(await source.getByRole('button').count() > 0, segmentation); assert.equal(await source.locator('[data-pos]').count() > 0, segmentation); assert.equal(await source.getAttribute('data-pos'), String(segmentation && pos)); if (segmentation && ruby && pos || !segmentation && ruby && !pos) await screenshot(page, `annotations-${Number(segmentation)}${Number(ruby)}${Number(pos)}`); } });
  await check('RBC-06-invalid-span-and-unknown-reading-preserve-source', async () => { await page.getByRole('checkbox', { name: 'ruby', exact: true }).check(); await page.getByRole('checkbox', { name: 'segmentation', exact: true }).check(); await page.getByRole('checkbox', { name: 'invalid-spans', exact: true }).check(); assert.equal(await source.textContent(), original); assert.equal(await source.locator('rt').count(), 0); assert.equal(await source.getByRole('button').count(), 0); await page.getByRole('checkbox', { name: 'invalid-spans', exact: true }).uncheck(); await page.getByRole('checkbox', { name: 'unknown-reading', exact: true }).check(); assert.equal(await source.locator('[data-token-index="0"] rt').count(), 0); await source.getByRole('button', { name: '図書館，读音 としょかん', exact: true }).click(); assert.equal(await page.getByTestId('token-index').textContent(), '2'); });
  await check('RBC-07-select-200-percent-and-popup-scroll-only', async () => { await page.evaluate(() => { document.body.style.fontSize = '26px'; }); await trigger.click(); const list = page.getByRole('listbox'); const before = await page.evaluate(() => ({ x: scrollX, y: scrollY })); await list.hover(); await page.mouse.wheel(0, 1000); await page.waitForTimeout(150); assert.ok(await list.evaluate(node => node.scrollTop) > 0); assert.deepEqual(await page.evaluate(() => ({ x: scrollX, y: scrollY })), before); const geometry = await page.getByRole('option').evaluateAll(nodes => nodes.map(node => { const b = node.getBoundingClientRect(); return { y: b.y, h: b.height }; })); for (let i = 1; i < geometry.length; i++) assert.equal(geometry[i].y - geometry[i - 1].y - geometry[i - 1].h, 2); await screenshot(page, 'select-long-scroll-200'); await page.keyboard.press('Escape'); });
  await check('RBC-08-lexical-failures-and-stale-identity', async () => {
    await page.evaluate(() => { document.body.style.fontSize = ''; });
    const status = page.getByRole('combobox', { name: '词法状态', exact: true });
    for (const state of ['pending', 'resource_unavailable', 'worker_failed', 'alignment_invalid', 'reading_unavailable']) {
      await status.click(); await page.getByRole('option', { name: state, exact: true }).click();
      await page.locator(`[data-lexical-state="${state}"]`).waitFor();
      assert.equal(await source.evaluate(node => { const clone=node.cloneNode(true);clone.querySelectorAll('rt').forEach(rt=>rt.remove());return clone.textContent; }), original);
      if (state !== 'reading_unavailable') assert.equal(await source.getByRole('button').count(), 0);
    }
    await status.click(); await page.getByRole('option', { name: 'worker_failed', exact: true }).click();
    await page.getByRole('button', { name: '重试分词', exact: true }).click(); assert.equal(await page.getByTestId('lexical-retries').textContent(), '1'); await page.locator('[data-lexical-state="pending"]').waitFor();
    await status.click(); await page.getByRole('option', { name: 'worker_failed', exact: true }).click(); await page.getByRole('checkbox', { name: 'stale-lexical', exact: true }).check();
    assert.equal(await page.locator('[data-lexical-state]').count(), 0); assert.ok(await source.getByRole('button').count() > 0);
  });
  await page.close();
  const appPage = async (role = 'main', width = 1040, height = 720, initialize = null) => {
    const p = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 }); p.setDefaultTimeout(8000); p.on('pageerror', error => errors.push(error.message));
    if (initialize) await p.addInitScript({ content: `window.__initializeControlledPort = (${initialize.toString()});` });
    await navigateWithProductionCsp(p, `http://127.0.0.1:${port}/tests/visual/index.html?role=${role}&scenario=reading`); await p.waitForFunction(() => window.uiHarness && document.getElementById('root')?.children.length > 0);
    await p.evaluate(() => Object.assign(document.getElementById('root').style, { width: '100%', height: '100%', left: '0', top: '0' }));
    // Explicit DTO transport fixtures preserve old UI obligations; actual Rust journeys run separately.
    await p.evaluate(() => window.uiHarness.prepareBindingJourneyFixture()); return p;
  };
  const emitSessionFrame = async (p, width = 160, height = 100) => p.evaluate(async ({width,height}) => {
    const port=window.uiHarness,projection=[...port.previewProjections.values()].find(value=>value.consumerId===port.state.session.sessionId&&value.state!=='stopped');
    if(!projection)throw Error('Explicit controlled session preview fixture is missing');
    const canvas=document.createElement('canvas');canvas.width=width;canvas.height=height;const context=canvas.getContext('2d');context.fillStyle='#476784';context.fillRect(0,0,width,height);
    const jpeg=await new Promise((resolve,reject)=>canvas.toBlob(blob=>blob?resolve(blob):reject(Error('Controlled JPEG encoding failed')),'image/jpeg'));
    port.savedPreviewUpdate={projection:{...projection,state:'live',capturedAtMs:'1792368001000'},frame:{leaseId:projection.leaseId,generation:projection.generation,sequence:'1',targetIdentityHash:projection.targetIdentityHash,capturedAtMs:'1792368001000',width,height,contentHash:'e'.repeat(64),encoding:'jpeg',payload:[...new Uint8Array(await jpeg.arrayBuffer())]}};
    port.emitPreview(port.savedPreviewUpdate);return projection.leaseId;
  }, {width,height});
  const prepareCover = async (p, mode = 'ready') => p.evaluate(async mode => {
    const port=window.uiHarness,gameId='controlled-cover-game',assetRef='managed-cover:'+('a'.repeat(64));
    const canvas=document.createElement('canvas');canvas.width=240;canvas.height=160;const context=canvas.getContext('2d');context.fillStyle='#547384';context.fillRect(0,0,240,160);
    const jpeg=await new Promise((resolve,reject)=>canvas.toBlob(blob=>blob?resolve(blob):reject(Error('Controlled cover JPEG encoding failed')),'image/jpeg'));
    const game={gameId,displayName:'Controlled local cover',originalTitle:'受控真实解码输入',sourceLanguage:'ja',coverAssetRef:mode==='missing'?null:assetRef,rowRevision:1,target:{environmentKind:'crossover',environmentKey:'controlled-cover-env',bottleLocator:'/controlled/Bottle',launchPath:null,canonicalExePath:'C:\\UI-test\\Game.exe',targetPathKey:'controlled-path',workingDirectory:null,arguments:[],architecture:'x86',verificationState:'verified'}};
    port.coverFixtures[gameId]={gameId:mode==='wrong-owner'?'another-game':gameId,assetRef,status:mode==='missing'?'missing':'ready',origin:'steam_library',contentHash:'a'.repeat(64),width:240,height:160,mimeType:'image/jpeg',payload:mode==='corrupt'?[1,2,3,4]:mode==='missing'?null:[...new Uint8Array(await jpeg.arrayBuffer())],errorCode:null};
    port.publish({games:[game]});return {gameId,assetRef};
  }, mode);
  await check('RBC-09-hud-common-controls-front-at-both-widths', async () => {
    const p = await appPage('hud', 560, 290);
    for (const width of [560, 500]) {
      await p.setViewportSize({ width, height: 290 });
      for (const name of ['可点击分词', '假名注音', '词性提示', '字幕内容模式：原文＋译文', '关闭自动翻译', '暂停自动输入', '重新翻译本句', '复制原文', '分析本句文法', '取消置顶', '切换紧凑布局', '隐藏浮窗']) { const button=p.getByRole('button',{name,exact:true}); assert.equal(await button.isVisible(),true);const box=await button.boundingBox();assert.ok(box.x>=0&&box.x+box.width<=width); }
      await screenshot(p, `hud-controls-${width}`);
    }
    await p.getByRole('button', { name: '更多阅读操作', exact: true }).click();
    assert.deepEqual(await p.getByRole('menuitem').allTextContents(), ['设置…','选择或更换游戏窗口…','解除窗口关联','恢复浮窗位置','断开游戏连接…']); await p.keyboard.press('Escape');
    const before=await p.evaluate(()=>structuredClone(window.uiHarness.state.current));await p.getByRole('button',{name:'复制原文',exact:true}).click();
    assert.deepEqual(await p.evaluate(()=>window.uiHarness.copiedReadingUnits.at(-1)),{unitId:before.unitId,textRevision:before.textRevision,text:before.sourceText});
    await p.getByRole('button',{name:'分析本句文法',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.state.learningObject.unit.unitId),before.unitId);
    await p.getByRole('button',{name:'字幕内容模式：原文＋译文',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.state.preferences.readingDisplayMode),'source');
    await p.getByRole('button',{name:'字幕内容模式：仅原文',exact:true}).click();await p.getByText('当前尚无完整译文，继续保留原文。',{exact:true}).waitFor();assert.equal(await p.getByTestId('source-text').count(),1);
    await p.evaluate(()=>{const port=window.uiHarness;port.publish({current:{...port.state.current,translations:[{translationId:'ui-translation',taskId:'ui-task',text:'可见译文',providerLabel:'Controlled UI',targetLanguage:'zh-CN',correction:null}]}});});
    await p.getByText('可见译文',{exact:true}).waitFor();assert.equal(await p.getByTestId('source-text').count(),0);await p.close();
  });
  await check('RBC-10-hud-native-drag-intent-and-local-scroll-menu', async () => {
    const p=await appPage('hud',560,290);await p.locator('[title="拖动浮窗"]').click();assert.equal(await p.evaluate(()=>window.uiHarness.nativeDragRequests),1);
    await p.getByRole('button',{name:'可点击分词',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.nativeDragRequests),1);
    await p.evaluate(()=>{const port=window.uiHarness;port.publish({current:{...port.state.current,sourceText:'長い原文を読む。\n'.repeat(80),tokens:[]}});});
    const footer=p.locator('footer').first(),reading=p.locator('[data-ui-scroll="reading"]');const bounds=await footer.boundingBox();await reading.hover();await p.mouse.wheel(0,700);await p.waitForFunction(()=>document.querySelector('[data-ui-scroll="reading"]').scrollTop>0);assert.deepEqual(await footer.boundingBox(),bounds);assert.deepEqual(await p.locator('html,body,#root,[data-ui-surface="hud"]').evaluateAll(nodes=>nodes.map(n=>n.scrollTop)),[0,0,0,0]);
    const more=p.getByRole('button',{name:'更多阅读操作',exact:true});await more.click();await p.getByRole('menu').waitFor();await p.mouse.move(0,0);assert.equal(await p.getByRole('menu').isVisible(),true);await p.keyboard.press('Escape');await p.waitForFunction(()=>document.activeElement?.getAttribute('aria-label')==='更多阅读操作');await screenshot(p,'hud-scroll-fixed-toolbar');await p.close();
  });
  await check('RBC-11-preview-frame-lease-pause-and-late-session-isolation', async () => {
    const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});
    // Automatic session leases originate in Rust; mounting only attaches the UI.
    await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===1);
    assert.equal(await preview.getByRole('button',{name:'开启窗口预览',exact:true}).count(),0);
    await p.evaluate(async()=>{const port=window.uiHarness,projection=[...port.previewProjections.values()].at(-1);if(projection.consumerId!==port.state.session.sessionId||port.previewRequests.length!==0)throw Error('Session preview must attach to the verified Rust lease without a client start');const canvas=document.createElement('canvas');canvas.width=160;canvas.height=100;canvas.getContext('2d').fillRect(0,0,160,100);const jpeg=await new Promise((resolve,reject)=>canvas.toBlob(value=>value?resolve(value):reject(Error('JPEG fixture encoding failed')),'image/jpeg'));const payload=[...new Uint8Array(await jpeg.arrayBuffer())];port.savedPreviewUpdate={projection:{...projection,state:'live',capturedAtMs:'1792368001000'},frame:{leaseId:projection.leaseId,generation:projection.generation,sequence:'1',targetIdentityHash:projection.targetIdentityHash,capturedAtMs:'1792368001000',width:160,height:100,contentHash:'e'.repeat(64),encoding:'jpeg',payload}};port.emitPreview(port.savedPreviewUpdate);});
    await preview.locator('img').waitFor();assert.deepEqual(await preview.locator('img').evaluate(async image=>{await image.decode();return{complete:image.complete,width:image.naturalWidth,height:image.naturalHeight};}),{complete:true,width:160,height:100},'Preview JPEG must decode under the actual production CSP');const image=await preview.locator('img').getAttribute('src');assert.match(image,/^blob:/);await preview.getByRole('button',{name:'暂停',exact:true}).click();await p.waitForFunction(()=>document.querySelector('[data-preview-state="paused"]'));assert.equal(await preview.locator('img').getAttribute('src'),image);
    assert.deepEqual(await preview.locator('img').evaluate(async image=>{await image.decode();return{complete:image.complete,width:image.naturalWidth,height:image.naturalHeight};}),{complete:true,width:160,height:100},'Paused preview must retain a decoded image');
    await p.evaluate(()=>window.uiHarness.emitPreview(window.uiHarness.savedPreviewUpdate));assert.equal(await preview.getAttribute('data-preview-state'),'paused');
    await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={...port.bindingFixture,sessionId:'ui-new-session'};port.publish({session:{...port.state.session,sessionId:'ui-new-session'}});});await p.waitForFunction(()=>window.uiHarness.previewSessionQueries.includes('ui-new-session'));await p.evaluate(()=>window.uiHarness.emitPreview(window.uiHarness.savedPreviewUpdate));assert.equal(await preview.locator('img').count(),0);await screenshot(p,'preview-new-session-clears-old-frame');await p.close();
  });
  await check('RBC-12-session-pagination-full-search-fixed-range-and-export', async () => {
    const p=await appPage();await p.evaluate(()=>{const port=window.uiHarness,id=port.state.session.sessionId;port.sessionRecordFixtures[`${id}:extraction`]=Array.from({length:160},(_,i)=>({kind:'extraction',sequence:String(i+1),eventId:`raw-${i+1}`,sourceId:i%2?'ui-stream':'unselected-stream',wireSequence:String(9007199254740993n+BigInt(i)),text:`台词${i+1}`,observedAtMs:1792368000000+i,reason:i%2?'selected':'unselected',bodyRefs:[],payloadRetained:true}));});await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await openSessionDataset(p,'全部提取');
    const rows=p.locator('details[data-flat-session-section][open] [data-record-sequence]');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===50);assert.equal(await rows.first().getAttribute('data-record-sequence'),'111');assert.equal(await rows.last().getAttribute('data-record-sequence'),'160');
    while(await p.getByRole('button',{name:'加载较早记录',exact:true}).count()){await p.getByRole('button',{name:'加载较早记录',exact:true}).click();await p.waitForFunction(()=>!document.querySelector('button[aria-busy=true]'));}
    assert.equal(await rows.count(),160);assert.deepEqual(await rows.evaluateAll(nodes=>nodes.map(n=>n.getAttribute('data-record-sequence'))),Array.from({length:160},(_,i)=>String(i+1)));
    const list=p.getByRole('log',{name:'全量提取文本列表',exact:true});await list.evaluate(node=>{node.scrollTop=250;});await p.getByRole('button',{name:/回到最新/}).waitFor();const scroll=await list.evaluate(node=>node.scrollTop);
    await p.evaluate(()=>{const port=window.uiHarness,key=`${port.state.session.sessionId}:extraction`,last=port.sessionRecordFixtures[key].at(-1);port.sessionRecordFixtures[key].push({...last,eventId:'raw-161',sequence:'161',text:'台词161'});});await p.getByRole('button',{name:/回到最新 · 1 条新内容/}).waitFor();assert.equal(await rows.count(),160);assert.equal(await list.evaluate(node=>node.scrollTop),scroll);await p.getByRole('button',{name:/回到最新 · 1 条新内容/}).click();await p.waitForFunction(()=>document.querySelector('[data-record-sequence="161"]'));assert.equal(await rows.last().getAttribute('data-record-sequence'),'161');
    await p.evaluate(()=>{const port=window.uiHarness,key=`${port.state.session.sessionId}:extraction`,last=port.sessionRecordFixtures[key].at(-1);port.sessionRecordFixtures[key].push({...last,eventId:'raw-162',sequence:'162',text:'台词162'});});await p.waitForFunction(()=>document.querySelector('[data-record-sequence="162"]'));assert.equal(await rows.last().getAttribute('data-record-sequence'),'162');
    await p.getByRole('textbox',{name:'搜索会话原文',exact:true}).fill('台词139');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===1);assert.equal(await rows.first().getAttribute('data-record-sequence'),'139');
    await p.getByRole('button',{name:'导出…',exact:true}).click();const dialog=p.getByRole('dialog',{name:'导出当前会话',exact:true});assert.equal(await dialog.getByRole('button',{name:'核对导出范围',exact:true}).isDisabled(),true);await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'核对导出范围',exact:true}).click();await dialog.getByRole('button',{name:'选择保存位置并导出',exact:true}).waitFor();await dialog.getByRole('button',{name:'取消',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.sessionExportActions.at(-1).action),'cancel');assert.equal(await rows.count(),1);await screenshot(p,'session-search-fixed-range');await p.close();
  });
  await check('RBC-13-hook-pages-failure-retry-late-search-and-hook-only-new-count', async () => {
    const p=await appPage();await p.evaluate(()=>{const port=window.uiHarness,id=port.state.session.sessionId;port.sessionRecordFixtures[`${id}:hook_events`]=Array.from({length:120},(_,i)=>({kind:'hook_event',sequence:String(i+1),eventId:`hook-${i+1}`,operationId:'controlled-operation',sessionId:id,sourceId:null,wireSequence:null,observedAtMs:1792368000000+i,level:i%2?'warning':'info',stage:i%3?'text_callback':'inject',code:'CONTROLLED_EVENT',message:`事件${i+1}`,lostCount:null,integrity:'complete_received_range'}));});await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await openSessionDataset(p,'Hook 日志');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===50);
    const rows=p.locator('details[data-flat-session-section][open] [data-record-sequence]');await p.evaluate(()=>{window.uiHarness.failNextSessionRecords=true;});await p.getByRole('button',{name:'加载较早记录',exact:true}).click();await p.getByRole('alert').filter({hasText:'Controlled session page load failed'}).waitFor();assert.equal(await rows.count(),50);await p.getByRole('button',{name:'重试',exact:true}).click();await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===100);
    await p.evaluate(()=>window.uiHarness.sessionRecordHoldSearch.push('事件7'));await p.getByRole('textbox',{name:'搜索 Hook 日志',exact:true}).fill('事件7');await p.waitForFunction(()=>window.uiHarness.sessionRecordHolds.has('事件7'));await p.getByRole('textbox',{name:'搜索 Hook 日志',exact:true}).fill('事件113');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===1);assert.equal(await rows.first().getAttribute('data-record-sequence'),'113');await p.evaluate(()=>window.uiHarness.sessionRecordHolds.get('事件7')());await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await rows.first().getAttribute('data-record-sequence'),'113');
    await p.evaluate(()=>{const port=window.uiHarness,key=`${port.state.session.sessionId}:hook_events`,last=port.sessionRecordFixtures[key].at(-1);port.sessionRecordFixtures[key].push({...last,eventId:'hook-121',sequence:'121',message:'事件113 新的'});});await p.getByRole('button',{name:/回到最新 · 1 条新内容/}).waitFor();assert.equal(await rows.count(),1);assert.equal(await rows.first().getAttribute('data-record-sequence'),'113');await screenshot(p,'hook-only-new-count-frozen-search');await p.close();
  });
  await check('RBC-14-preview-permission-does-not-change-hook-session', async () => {
    const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');const before=await p.evaluate(()=>structuredClone(window.uiHarness.state.session)),preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===1);
    await p.evaluate(()=>{const port=window.uiHarness,projection=[...port.previewProjections.values()].at(-1);port.emitPreview({projection:{...projection,state:'permission_denied',permission:'denied',detail:'Controlled permission denial'},frame:null});});await p.waitForFunction(()=>document.querySelector('[data-preview-state="permission_denied"]'));assert.equal(await preview.getAttribute('data-preview-state'),'permission_denied');assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.session),before);await preview.getByText('授权说明…',{exact:true}).click();await screenshot(p,'preview-denied-preserves-hook-session');await p.close();
  });
  await check('RBC-15-hud-natural-extent-grows-shrinks-and-deduplicates', async () => {
    const p=await appPage('hud',560,290);await p.waitForFunction(()=>window.uiHarness.hudFitRequests.length>0);await p.evaluate(()=>document.fonts.ready);const initial=await p.evaluate(()=>window.uiHarness.hudFitRequests.at(-1));
    await p.evaluate(()=>{const port=window.uiHarness;port.publish({current:{...port.state.current,sourceText:'長い原文を読む。\n'.repeat(40),tokens:[],translations:[]}});});await p.waitForFunction(start=>window.uiHarness.hudFitRequests.at(-1)>start,initial);const large=await p.evaluate(()=>window.uiHarness.hudFitRequests.at(-1));
    await p.evaluate(()=>{const port=window.uiHarness;port.publish({current:{...port.state.current,sourceText:'短い。',tokens:[],translations:[]}});});await p.waitForFunction(previous=>window.uiHarness.hudFitRequests.at(-1)<previous,large);const count=await p.evaluate(()=>window.uiHarness.hudFitRequests.length);
    await p.evaluate(()=>{const port=window.uiHarness;port.publish({diagnostics:[...port.state.diagnostics]});});await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await p.evaluate(()=>window.uiHarness.hudFitRequests.length),count);assert.equal(await p.evaluate(()=>window.uiHarness.capabilityFixture.geometry.logicalBounds.height>=140&&window.uiHarness.capabilityFixture.geometry.logicalBounds.height<=366),true);await p.close();
    // The real HUD/query path must not fit temporary query-only feedback when
    // immutable source and translation content remain unchanged. Keep the
    // original long/short/dedup assertions above, and emulate host resize below.
    const stableHud = await appPage('hud', 560, 290, port => {
      port.state.preferences = { ...port.state.preferences, segmentationEnabled: false, showRuby: false, showPos: false };
      port.state.current = { ...port.state.current, sourceText: '短い原文。', tokens: [], translations: [{ translationId: 'held-refresh-translation', taskId: 'held-refresh-task', text: '简短译文。', providerLabel: 'controlled query', targetLanguage: 'zh-CN', correction: null }] };
      port.state.history.units = [port.state.current];
      const fit = port.fitHudContent.bind(port);
      port.fitHudContent = async height => { const root = document.getElementById('root'); if (root) root.style.height = `${Math.max(140, Math.min(366, height))}px`; return fit(height); };
    });
    await stableHud.evaluate(() => document.fonts.ready);
    await stableHud.waitForFunction(() => window.uiHarness.readingProjectionQueries.length > 0 && [...window.uiHarness.readingProjectionActive.values()].every(value => value === 0));
    const queryObservations = [];
    const extent = () => stableHud.evaluate(() => { const reading = document.querySelector('[data-ui-scroll="reading"]'), port = window.uiHarness; return { height: reading.firstElementChild.getBoundingClientRect().height, fits: [...port.hudFitRequests], source: port.state.current.sourceText, translations: port.state.current.translations.map(item => item.text) }; });
    for (const width of [560, 500]) {
      await stableHud.setViewportSize({ width, height: 290 });
      await stableHud.evaluate(() => { const port = window.uiHarness; delete port.readingProjectionFixtures[JSON.stringify([port.state.current.unitId, port.state.current.textRevision])]; port.publish({ commitRevision: port.state.commitRevision + 1, tasks: [] }); });
      await stableHud.waitForFunction(() => [...window.uiHarness.readingProjectionActive.values()].every(value => value === 0) && !document.body.textContent.includes('翻译状态更新中'));
      await stableHud.waitForTimeout(180);
      const stable = await extent();
      for (let cycle = 0; cycle < 3; cycle++) {
        const holdKey = `stable-refresh-${width}-${cycle}`;
        await stableHud.evaluate(holdKey => { const port = window.uiHarness; port.readingProjectionPlans.push({ holdKey }); port.publish({ commitRevision: port.state.commitRevision + 1, diagnostics: [...port.state.diagnostics] }); }, holdKey);
        await stableHud.waitForFunction(holdKey => window.uiHarness.readingProjectionPending.has(holdKey), holdKey);
        await stableHud.waitForTimeout(80);
        const pending = await extent();
        queryObservations.push({ kind: 'completed-query-pending', width, cycle, stable, pending });
        await fs.writeFile(path.join(out, 'hud-refresh-query-observations.json'), JSON.stringify(queryObservations, null, 2) + '\n');
        assert.equal(pending.height, stable.height, 'Query-only pending feedback cannot change the HUD natural content height');
        assert.deepEqual(pending.fits, stable.fits, 'Unrelated commit with held query must not submit a fit request');
        assert.equal(pending.source, stable.source); assert.deepEqual(pending.translations, stable.translations);
        const feedback = stableHud.getByRole('status').filter({ hasText: '翻译状态更新中，以下保留上次读取结果。' });
        assert.equal(await feedback.getAttribute('aria-label'), '翻译状态');
        await feedback.evaluate(node => { window.controlledHudQueryFeedbackNode = node; });
        assert.equal(await feedback.evaluate(node => getComputedStyle(node).position), 'absolute');
        assert.match(await feedback.ariaSnapshot(), /翻译状态更新中/);
        await stableHud.evaluate(holdKey => window.uiHarness.releaseReadingProjection(holdKey), holdKey);
        await stableHud.waitForFunction(() => [...window.uiHarness.readingProjectionActive.values()].every(value => value === 0) && !document.body.textContent.includes('翻译状态更新中，以下保留上次读取结果。'));
        await stableHud.waitForTimeout(80);
        const settled = await extent();
        queryObservations.push({ kind: 'completed-query-settled', width, cycle, settled });
        assert.equal(settled.height, stable.height); assert.deepEqual(settled.fits, stable.fits);
        assert.equal(await stableHud.getByRole('status', { name: '翻译状态', exact: true }).evaluate(node => node === window.controlledHudQueryFeedbackNode && node.textContent === ''), true, 'The layout-stable live status slot stays mounted after the query settles');
      }
      // These labels are recorded original-source wrap boundaries, not fixed
      // product heights. The same immutable running/failed task must not reflow
      // merely because an unrelated commit refreshes its as-of query.
      for (const taskStatus of ['running', 'failed']) {
        const length = taskStatus === 'running' ? (width === 560 ? 80 : 70) : (width === 560 ? 25 : 20);
        const providerLabel = '前缀反例引擎' + '甲'.repeat(length);
        await stableHud.evaluate(({ providerLabel, taskStatus }) => {
          const port = window.uiHarness, unit = port.state.current, commitRevision = port.state.commitRevision + 1;
          const task = { taskId: 'held-refresh-task', unitId: unit.unitId, textRevision: unit.textRevision, purpose: 'translation', status: taskStatus, errorCode: taskStatus === 'failed' ? '受控错误受控错误受控错误' : null, partialText: null, providerLabel, attemptId: 'held-refresh-attempt' };
          port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = { profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit, translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines: [{ scopeKey: 'global', bindingId: 'held-refresh-binding', engineId: 'held-refresh-engine', providerLabel, status: { kind: 'current', task } }] } };
          port.publish({ commitRevision, tasks: [task] });
        }, { providerLabel, taskStatus });
        await stableHud.waitForFunction(() => [...window.uiHarness.readingProjectionActive.values()].every(value => value === 0) && !document.body.textContent.includes('翻译状态更新中'));
        await stableHud.waitForTimeout(80); const taskStable = await extent();
        const holdKey = `task-prefix-${width}-${taskStatus}`;
        await stableHud.evaluate(holdKey => { const port = window.uiHarness, commitRevision = port.state.commitRevision + 1; port.readingProjectionFixtures[JSON.stringify([port.state.current.unitId, port.state.current.textRevision])].commitRevision = commitRevision; port.readingProjectionPlans.push({ holdKey }); port.publish({ commitRevision }); }, holdKey);
        await stableHud.waitForFunction(holdKey => window.uiHarness.readingProjectionPending.has(holdKey), holdKey); await stableHud.waitForTimeout(80);
        const taskPending = await extent(); queryObservations.push({ kind: 'task-query-pending', width, taskStatus, providerLabel, stable: taskStable, pending: taskPending });
        await fs.writeFile(path.join(out, 'hud-refresh-query-observations.json'), JSON.stringify(queryObservations, null, 2) + '\n');
        assert.equal(taskPending.height, taskStable.height, 'A query-only stale prefix must not wrap a HUD running/failed task'); assert.deepEqual(taskPending.fits, taskStable.fits);
        assert.equal(taskPending.source, taskStable.source); assert.deepEqual(taskPending.translations, taskStable.translations);
        assert.equal(await stableHud.getByText(`上次读取 · ${providerLabel} · ${taskStatus === 'running' ? '翻译中' : '翻译失败：受控错误受控错误受控错误'}`, { exact: true }).isVisible(), true, 'Existing task/owner status text remains present');
        await stableHud.evaluate(holdKey => window.uiHarness.releaseReadingProjection(holdKey), holdKey);
        await stableHud.waitForFunction(() => [...window.uiHarness.readingProjectionActive.values()].every(value => value === 0) && !document.body.textContent.includes('翻译状态更新中')); await stableHud.waitForTimeout(80);
        const taskSettled = await extent(); queryObservations.push({ kind: 'task-query-settled', width, taskStatus, settled: taskSettled }); assert.equal(taskSettled.height, taskStable.height); assert.deepEqual(taskSettled.fits, taskStable.fits);
      }
      assert.ok(await stableHud.evaluate(() => [...window.uiHarness.readingProjectionMaxActive.values()].every(value => value === 1)), 'HUD opt-in must retain active single-flight querying');
      await screenshot(stableHud, `hud-refresh-stable-${width}`);
    }
    // Real query failure keeps the visible notice, the old task and explicit
    // retry. A failed read is meaningful content, so it may change the extent.
    await stableHud.evaluate(() => { const port = window.uiHarness, commitRevision = port.state.commitRevision + 1; port.readingProjectionFixtures[JSON.stringify([port.state.current.unitId, port.state.current.textRevision])].commitRevision = commitRevision; port.readingProjectionPlans.push({ error: 'Controlled HUD query read failed' }); port.publish({ commitRevision }); });
    const queryError = stableHud.getByRole('alert').filter({ hasText: 'Controlled HUD query read failed' }); await queryError.waitFor(); assert.equal(await queryError.isVisible(), true);
    const retry = stableHud.getByRole('button', { name: '重新读取翻译状态', exact: true }); assert.equal(await retry.isVisible(), true); await retry.click();
    await stableHud.waitForFunction(() => !document.body.textContent.includes('翻译状态读取失败：'));
    assert.equal(await stableHud.getByText(/前缀反例引擎.*翻译失败：受控错误受控错误受控错误/).isVisible(), true);
    await fs.writeFile(path.join(out, 'hud-refresh-query-observations.json'), JSON.stringify(queryObservations, null, 2) + '\n');
    await stableHud.close();
    // The default shared presentation still visibly reports a background read.
    const main = await appPage(); await main.getByRole('button', { name: '当前会话', exact: true }).click();await openSessionDataset(main,'正文历史');
    await main.waitForFunction(() => window.uiHarness.readingProjectionQueries.length > 0 && [...window.uiHarness.readingProjectionActive.values()].every(value => value === 0));
    await main.evaluate(() => { const port = window.uiHarness; port.readingProjectionPlans.push({ holdKey: 'main-refresh-default' }); port.publish({ commitRevision: port.state.commitRevision + 1 }); });
    await main.waitForFunction(() => window.uiHarness.readingProjectionPending.has('main-refresh-default'));
    const notice = main.getByText('翻译状态更新中，以下保留上次读取结果。', { exact: true });
    assert.equal(await notice.isVisible(), true); assert.ok((await notice.boundingBox()).height > 1);
    assert.notEqual(await notice.evaluate(node => getComputedStyle(node).position), 'absolute');
    await main.evaluate(() => window.uiHarness.releaseReadingProjection('main-refresh-default'));
    await main.waitForFunction(() => !document.body.textContent.includes('翻译状态更新中，以下保留上次读取结果。'));
    await main.close();
  });
  await check('RBC-16-full-session-scroll-owner-reaches-preview-and-history', async () => {
    const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');
    const scroller=p.locator('[data-ui-scroll="main-content"]').filter({visible:true});assert.equal(await scroller.count(),1);
    const geometry=await scroller.evaluate(node=>({height:node.clientHeight,scroll:node.scrollHeight}));assert.ok(geometry.scroll>geometry.height,'Full current-session content must remain reachable beyond the native viewport');
    const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await preview.getByRole('button',{name:'暂停',exact:true}).scrollIntoViewIfNeeded();
    const viewport=await scroller.boundingBox(),button=await preview.getByRole('button',{name:'暂停',exact:true}).boundingBox();assert.ok(button.y>=viewport.y&&button.y+button.height<=viewport.y+viewport.height,'Preview controls must be reachable through the real session scroll owner');
    const records=p.getByRole('log',{name:'正文历史列表',exact:true});await records.scrollIntoViewIfNeeded();const box=await records.boundingBox();assert.ok(box.y<viewport.y+viewport.height&&box.y+box.height>viewport.y,'Persisted body history must not be clipped below the session panel');
    assert.equal(await p.getByRole('tablist',{name:'当前会话内容',exact:true}).count(),0,'Mutually exclusive session tabs are retired');assert.equal(await p.getByRole('article',{name:'当前原文与译文',exact:true}).evaluate(node=>node.closest('details')===null),true,'Current text remains independent of every historical disclosure');await screenshot(p,'session-full-window-reaches-history');await p.close();
  });
  await check('RBC-17-compact-preference-keeps-base-fonts-with-one-effective-scale',async()=>{
    const p=await appPage('settings',1040,760);await p.getByRole('button',{name:'显示与快捷键',exact:true}).click();await p.getByRole('combobox',{name:'阅读密度',exact:true}).click();await p.getByRole('option',{name:'紧凑 · 500 / 16 / 22',exact:true}).click();await p.getByRole('button',{name:'保存阅读偏好',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.state.preferences.overrides['hud.compact']===true);const preferences=await p.evaluate(()=>window.uiHarness.state.preferences);assert.equal(preferences.sourceFontPt,18);assert.equal(preferences.translationFontPt,24);assert.equal(preferences.hudWidthPt,500);
    const hud=await appPage('hud',500,290);await hud.evaluate(preferences=>{const port=window.uiHarness;port.publish({preferences,current:{...port.state.current,translations:[{translationId:'controlled-font-result',taskId:'controlled-font-task',text:'用于检查有效字号的受控译文',providerLabel:'受控排版输入',targetLanguage:'zh-CN',correction:null}]}});},preferences);await hud.waitForFunction(()=>getComputedStyle(document.querySelector('[data-testid=source-text]')).fontSize==='16px');assert.equal(await hud.getByRole('article').locator('p').first().evaluate(node=>getComputedStyle(node).fontSize),'22px');await hud.close();await p.close();
  });

  await check('RBC-18-scoped-preference-mask-absent-head-cas-and-inheritance',async()=>{
    const p=await appPage('settings',1040,760,port=>{
      port.state.games=[{gameId:'scope-game',displayName:'受控范围游戏',originalTitle:null,sourceLanguage:'ja',coverAssetRef:null,rowRevision:1,target:{environmentKind:'unresolved',environmentKey:'',bottleLocator:null,launchPath:null,canonicalExePath:null,targetPathKey:null,workingDirectory:null,arguments:[],architecture:null,verificationState:'unresolved'}}];
      port.state.preferences.overrides={'hud.compact':true,'hud.alwaysOnTop':false};
    });
    await p.getByRole('button',{name:'显示与快捷键',exact:true}).click();
    const scope=p.getByRole('combobox',{name:'阅读偏好作用范围',exact:true});await scope.click();await p.getByRole('option',{name:'仅此游戏 · 受控范围游戏',exact:true}).click();
    await p.getByText('此游戏继承全局默认',{exact:true}).waitFor();
    assert.equal(await p.getByRole('combobox',{name:'阅读密度',exact:true}).getAttribute('data-select-value'),'compact');
    assert.equal(await p.getByRole('checkbox',{name:'游戏在前台时保持浮窗可见',exact:true}).isChecked(),false);
    assert.equal(await p.evaluate(()=>window.uiHarness.controlledScope('game:scope-game').effective.scopeKey),'global');
    const font=p.getByRole('spinbutton',{name:'原文字号（pt）',exact:true});await font.fill('28');
    await p.getByRole('button',{name:'保存阅读偏好',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.scopedPreferenceSaves.get('game:scope-game')?.headRevision===1);
    const first=await p.evaluate(()=>({entry:window.uiHarness.scopedPreferenceSaves.get('game:scope-game'),runtime:window.uiHarness.state.preferences,command:window.uiHarness.commands.filter(item=>item.command.kind==='save_scoped_preferences').at(-1).command}));
    assert.equal(first.command.payload.expectedHeadRevision,null);assert.equal(first.entry.saved.scopeKey,'game:scope-game');assert.deepEqual(first.entry.saved.overrides,{sourceFontPt:true});assert.equal(first.entry.saved.sourceFontPt,28);assert.equal(first.runtime.sourceFontPt,18);
    await p.waitForFunction(id=>document.getElementById(id)?.value==='28',await font.getAttribute('id'));
    await p.evaluate(()=>window.uiHarness.scopedPreferenceSaves.get('game:scope-game').headRevision=2);await font.fill('30');await p.getByRole('button',{name:'保存阅读偏好',exact:true}).click();await p.getByRole('alert').filter({hasText:'preference_head_conflict'}).waitFor();
    assert.equal(await font.inputValue(),'30');assert.equal(await p.evaluate(()=>window.uiHarness.scopedPreferenceSaves.get('game:scope-game').saved.sourceFontPt),28);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_scoped_preferences').length),2);
    await p.getByRole('button',{name:'放弃修改',exact:true}).click();await p.waitForFunction(id=>document.getElementById(id)?.value==='28',await font.getAttribute('id'));await p.getByRole('button',{name:'恢复继承全局',exact:true}).click();await p.getByRole('button',{name:'保存阅读偏好',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.scopedPreferenceSaves.get('game:scope-game')?.headRevision===3);
    const inherited=await p.evaluate(()=>({saved:window.uiHarness.scopedPreferenceSaves.get('game:scope-game').saved,view:window.uiHarness.controlledScope('game:scope-game')}));assert.deepEqual(inherited.saved.overrides,{});assert.equal(inherited.view.effective.sourceFontPt,18);assert.equal(inherited.view.effective.overrides['hud.compact'],true);assert.equal(inherited.view.effective.overrides['hud.alwaysOnTop'],false);await p.close();
  });
  await check('RBC-19-scope-read-failure-preserves-dirty-draft-until-explicit-discard',async()=>{
    const failed=await appPage('settings',1040,760,port=>port.scopedPreferenceResponses.push({error:'Controlled scope read failed'}));await failed.getByRole('button',{name:'显示与快捷键',exact:true}).click();await failed.getByRole('alert').filter({hasText:'Controlled scope read failed'}).waitFor();assert.equal(await failed.getByRole('spinbutton',{name:'原文字号（pt）',exact:true}).count(),0);await failed.getByRole('button',{name:'重新读取偏好来源',exact:true}).click();await failed.getByRole('spinbutton',{name:'原文字号（pt）',exact:true}).waitFor();assert.equal(await failed.getByRole('spinbutton',{name:'原文字号（pt）',exact:true}).inputValue(),'18');await failed.close();
    const p=await appPage('settings',1040,760);await p.getByRole('button',{name:'显示与快捷键',exact:true}).click();const font=p.getByRole('spinbutton',{name:'原文字号（pt）',exact:true});await font.fill('31');await p.evaluate(()=>window.uiHarness.scopedPreferenceResponses.push({error:'Controlled later scope read failed'}));await p.getByRole('button',{name:'放弃修改',exact:true}).click();await p.getByRole('alert').filter({hasText:'Controlled later scope read failed'}).waitFor();assert.equal(await font.inputValue(),'31');await p.getByRole('button',{name:'重新读取偏好来源',exact:true}).click();await p.getByRole('alert').filter({hasText:'Controlled later scope read failed'}).waitFor({state:'hidden'});assert.equal(await font.inputValue(),'31');await p.getByRole('button',{name:'放弃修改',exact:true}).click();await p.waitForFunction(id=>document.getElementById(id)?.value==='18',await font.getAttribute('id'));assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_scoped_preferences').length),0);await p.close();
  });
  await check('RBC-20-native-capability-failure-single-flight-and-late-generation',async()=>{
    const failed=await appPage('settings',1040,760,port=>port.capabilityResponses.push({error:'Controlled native capability failure'}));await failed.getByRole('button',{name:'显示与快捷键',exact:true}).click();await failed.getByRole('alert').filter({hasText:'Controlled native capability failure'}).waitFor();await failed.getByRole('button',{name:'重试平台状态',exact:true}).click();await failed.getByText('原生磨砂材质',{exact:true}).waitFor();await failed.close();
    const p=await appPage('settings',1040,760,port=>{port.capabilityResponses.push({holdKey:'old-capability'});});await p.getByRole('button',{name:'显示与快捷键',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.capabilityPending.has('old-capability'));
    await p.evaluate(()=>{const port=window.uiHarness,next=structuredClone(port.capabilityFixture);next.material.effective='opaque_fallback';next.material.effectiveReduceTransparency=true;port.capabilityResponses.push({value:next});port.publish({preferences:{...port.state.preferences,showRuby:false}});});await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await p.evaluate(()=>window.uiHarness.capabilityQueries),1);
    await p.evaluate(()=>window.uiHarness.releaseCapability('old-capability'));await p.getByText('实色回退',{exact:true}).waitFor();assert.equal(await p.getByText('原生磨砂材质',{exact:true}).count(),0);assert.equal(await p.evaluate(()=>window.uiHarness.capabilityQueries),2);
    await p.evaluate(()=>window.uiHarness.capabilityResponses.push({error:'Controlled later capability failure'}));await p.getByRole('button',{name:'恢复浮窗位置',exact:true}).click();await p.getByRole('alert').filter({hasText:'Controlled later capability failure'}).waitFor();assert.equal(await p.getByText('实色回退',{exact:true}).isVisible(),true);await p.getByText('平台状态待确认',{exact:true}).waitFor();await p.getByRole('button',{name:'重试平台状态',exact:true}).click();await p.getByText('原生磨砂材质',{exact:true}).waitFor();await p.close();
  });
  await check('RBC-21-save-in-flight-keeps-newer-draft-and-advances-cas',async()=>{
    const p=await appPage('settings',1040,760);await p.getByRole('button',{name:'显示与快捷键',exact:true}).click();const font=p.getByRole('spinbutton',{name:'原文字号（pt）',exact:true});await font.fill('28');await p.evaluate(()=>window.uiHarness.preferenceSaveHoldKeys.push('saving-28'));await p.getByRole('button',{name:'保存阅读偏好',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.preferenceSavePending.has('saving-28'));await font.fill('30');await p.evaluate(()=>window.uiHarness.releasePreferenceSave('saving-28'));await p.waitForFunction(()=>window.uiHarness.scopedPreferenceSaves.get('global')?.headRevision===1);await p.getByRole('button',{name:'保存阅读偏好',exact:true}).waitFor({state:'visible'});await p.waitForFunction(()=>!document.querySelector('button[aria-busy=true]'));assert.equal(await font.inputValue(),'30');assert.equal(await p.evaluate(()=>window.uiHarness.scopedPreferenceSaves.get('global').saved.sourceFontPt),28);assert.equal(await p.getByRole('button',{name:'保存阅读偏好',exact:true}).isEnabled(),true);await p.getByRole('button',{name:'保存阅读偏好',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.scopedPreferenceSaves.get('global')?.headRevision===2);assert.equal(await p.evaluate(()=>window.uiHarness.scopedPreferenceSaves.get('global').saved.sourceFontPt),30);assert.deepEqual(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_scoped_preferences').map(item=>item.command.payload.expectedHeadRevision)),[null,1]);await p.close();
  });
  await check('RBC-22-final-page-refresh-retry-keeps-fixed-range-and-scroll',async()=>{
    const p=await appPage();await p.evaluate(()=>{const port=window.uiHarness,id=port.state.session.sessionId;port.sessionRecordFixtures[`${id}:extraction`]=Array.from({length:30},(_,i)=>({kind:'extraction',sequence:String(i+1),eventId:`last-page-${i+1}`,sourceId:'ui-stream',wireSequence:String(i+1),text:`固定范围原文${i+1}`,observedAtMs:1792368000000+i,reason:'selected',bodyRefs:[],payloadRetained:true}));});await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await openSessionDataset(p,'全部提取');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===30);assert.equal(await p.getByRole('button',{name:'加载较早记录',exact:true}).count(),0);
    const list=p.getByRole('log',{name:'全量提取文本列表',exact:true});await p.getByRole('button',{name:'跟随新内容',exact:true}).click();await list.evaluate(node=>node.scrollTop=120);const before=await list.evaluate(node=>({scroll:node.scrollTop,highWatermark:node.dataset.highWatermark,rows:[...node.querySelectorAll('[data-record-sequence]')].map(row=>row.dataset.recordSequence)}));await p.evaluate(()=>window.uiHarness.failNextSessionRecords=true);await p.getByRole('alert').filter({hasText:'Controlled session page load failed'}).waitFor();const failedCursor=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.at(-1).cursor);assert.ok(failedCursor);await p.getByRole('button',{name:'重试',exact:true}).click();await p.getByRole('alert').filter({hasText:'Controlled session page load failed'}).waitFor({state:'hidden'});assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.at(-1).cursor),failedCursor);assert.deepEqual(await list.evaluate(node=>({scroll:node.scrollTop,highWatermark:node.dataset.highWatermark,rows:[...node.querySelectorAll('[data-record-sequence]')].map(row=>row.dataset.recordSequence)})),before);await p.close();
  });
  await check('RBC-23-bound-window-activation-result-and-unbind-preserves-capture',async()=>{
    const p=await appPage();assert.equal(await p.evaluate(()=>window.uiHarness.bindingQueries),0);await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');const show=p.getByRole('button',{name:'显示游戏',exact:true});await show.waitFor();await p.waitForFunction(()=>[...document.querySelectorAll('button')].some(button=>button.textContent==='显示游戏'&&!button.disabled));await p.evaluate(()=>{const port=window.uiHarness,b=port.bindingFixture;port.activationResponses.push({value:{sessionId:b.sessionId,targetIdentitySha256:b.targetIdentitySha256,state:'visible_not_foreground',wasMinimized:true,showWasVisible:false,setForegroundAccepted:false,visibleAfter:true,foregroundAfter:false,observedAtMs:'1792368000001'}});});await show.click();await p.getByText('游戏窗口已显示，但系统未允许切到前台。请手动点击游戏窗口。',{exact:true}).waitFor();assert.equal(await p.evaluate(()=>window.uiHarness.activations.length),1);const before=await p.evaluate(()=>structuredClone(window.uiHarness.state.session));await openWindowManagement(p);await p.getByRole('button',{name:'解除窗口关联',exact:true}).click();await p.getByText('尚无可核验的关联窗口',{exact:true}).waitFor();assert.equal(await show.isDisabled(),true);assert.equal(await p.getByRole('button',{name:'开启窗口预览',exact:true}).count(),0);assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.session),before);assert.equal(await p.getByText('游戏窗口已显示，但系统未允许切到前台。请手动点击游戏窗口。',{exact:true}).count(),0);await p.close();
  });
  await check('RBC-24-activation-wrong-target-and-late-session-cannot-claim-success',async()=>{
    const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');const show=p.getByRole('button',{name:'显示游戏',exact:true});await p.waitForFunction(()=>[...document.querySelectorAll('button')].some(button=>button.textContent==='显示游戏'&&!button.disabled));
    await p.evaluate(()=>{const port=window.uiHarness,b=port.bindingFixture;port.activationResponses.push({value:{sessionId:b.sessionId,targetIdentitySha256:'c'.repeat(64),state:'visible_not_foreground',wasMinimized:false,showWasVisible:true,setForegroundAccepted:false,visibleAfter:true,foregroundAfter:false,observedAtMs:'1792368000001'}});});await show.click();await p.getByRole('alert').filter({hasText:'显示回执不属于当前关联目标'}).waitFor();assert.equal(await p.getByText('游戏窗口已显示，但系统未允许切到前台。请手动点击游戏窗口。',{exact:true}).count(),0);
    await p.evaluate(()=>{const port=window.uiHarness,b=port.bindingFixture;port.activationResponses.push({holdKey:'old-activation',value:{sessionId:b.sessionId,targetIdentitySha256:b.targetIdentitySha256,state:'visible_not_foreground',wasMinimized:false,showWasVisible:true,setForegroundAccepted:false,visibleAfter:true,foregroundAfter:false,observedAtMs:'1792368000002'}});});await show.click();await p.waitForFunction(()=>window.uiHarness.activationPending.has('old-activation'));await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={...port.bindingFixture,sessionId:'new-activation-session',targetIdentitySha256:'d'.repeat(64)};port.publish({session:{...port.state.session,sessionId:'new-activation-session'}});});await p.waitForFunction(()=>window.uiHarness.bindingQueries>=3);await p.evaluate(()=>window.uiHarness.releaseActivation('old-activation'));await p.waitForFunction(()=>!document.querySelector('button[aria-busy=true]'));assert.equal(await p.getByText('游戏窗口已显示，但系统未允许切到前台。请手动点击游戏窗口。',{exact:true}).count(),0);assert.equal(await p.getByRole('alert').filter({hasText:'显示回执不属于当前关联目标'}).count(),0);assert.equal(await p.evaluate(()=>window.uiHarness.state.session.sessionId),'new-activation-session');await p.close();
  });
  await check('RBC-25-hud-tooltips-use-only-native-effective-shortcuts',async()=>{
    const p=await appPage('hud',560,290,port=>{port.capabilityFixture.shortcuts.bindings=[{intent:'toggle_hud',desired:'Alt+Super+H',effective:'Alt+Super+J',scopeKey:'global',errorCode:'CONTROLLED_CONFLICT',errorMessage:'Requested key unavailable; previous key remains effective'},{intent:'toggle_capture_pause',desired:'Alt+Super+P',effective:'Alt+Super+K',scopeKey:'global',errorCode:null,errorMessage:null},{intent:'cycle_reading_display_mode',desired:'Alt+Super+M',effective:'Alt+Super+L',scopeKey:'global',errorCode:null,errorMessage:null}];});
    const hide=p.getByRole('button',{name:'隐藏浮窗',exact:true}),pause=p.getByRole('button',{name:'暂停自动输入',exact:true}),mode=p.getByRole('button',{name:'字幕内容模式：原文＋译文',exact:true});await p.waitForFunction(()=>document.querySelector('button[aria-label="隐藏浮窗"]')?.title.includes('⌥ ⌘ J'));
    assert.equal(await hide.getAttribute('title'),'隐藏浮窗 · ⌥ ⌘ J');assert.equal(await pause.getAttribute('title'),'暂停自动输入 · ⌥ ⌘ K');assert.equal(await mode.getAttribute('title'),'切换原文与译文显示 · ⌥ ⌘ L');
    const count=await p.evaluate(()=>window.uiHarness.capabilityQueries);await p.evaluate(()=>{const port=window.uiHarness;for(let index=0;index<12;index++)port.publish({current:{...port.state.current,unitId:`title-unit-${index}`}});});await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await p.evaluate(()=>window.uiHarness.capabilityQueries),count);
    await p.evaluate(()=>{window.uiHarness.capabilityResponses.push({error:'Controlled HUD shortcut read failure'});window.dispatchEvent(new Event('focus'));});await p.waitForFunction(()=>document.querySelector('button[aria-label="隐藏浮窗"]')?.title.includes('待核对'));assert.equal((await hide.getAttribute('title')).includes('⌘'),false);await p.close();
  });
  await check('RBC-26-late-unbind-success-and-failure-do-not-clear-next-session',async()=>{
    for(const fail of [false,true]) {
      const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await openWindowManagement(p);await p.getByRole('button',{name:'解除窗口关联',exact:true}).waitFor();await p.evaluate(fail=>window.uiHarness.unbindResponses.push({holdKey:'old-unbind',...(fail?{error:'Controlled old unbind failure'}:{})}),fail);await openWindowManagement(p);await p.getByRole('button',{name:'解除窗口关联',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.unbindPending.has('old-unbind'));
      await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={state:'bound',sessionId:'next-unbind-session',targetIdentitySha256:'d'.repeat(64),windowTitle:'Next explicit controlled window',verifiedAtMs:'1792368001000',errorCode:null,detail:'New independent verified binding fixture'};port.publish({session:{...port.state.session,sessionId:'next-unbind-session'}});});await p.getByRole('region',{name:'运行窗口预览',exact:true}).getByText('Next explicit controlled window',{exact:true}).waitFor();const queries=await p.evaluate(()=>window.uiHarness.bindingQueries);await p.evaluate(()=>window.uiHarness.releaseUnbind('old-unbind'));await p.waitForFunction(()=>!document.querySelector('button[aria-busy=true]'));assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isEnabled(),true);assert.equal(await p.getByRole('region',{name:'运行窗口预览',exact:true}).count(),1);assert.equal(await p.getByRole('button',{name:'开启窗口预览',exact:true}).count(),0);assert.equal(await p.getByRole('alert').filter({hasText:'Controlled old unbind failure'}).count(),0);assert.equal(await p.evaluate(()=>window.uiHarness.bindingQueries),queries);assert.equal(await p.evaluate(()=>window.uiHarness.bindingFixture.sessionId),'next-unbind-session');await p.close();
    }
  });
  await check('RBC-27-proof-busy-keeps-last-bound-preview-and-real-stale-clears-it',async()=>{
    const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===1);const lease=await p.evaluate(()=>[...window.uiHarness.previewProjections.keys()].at(-1));
    await p.evaluate(()=>{const port=window.uiHarness;port.bindingResponses.push({value:{...port.bindingFixture,state:'unavailable',errorCode:'window_proof_busy',detail:'Controlled concurrent proof operation'}});});await p.getByText('窗口正在重新核对，暂保留上次确认的关联。',{exact:false}).waitFor();assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isEnabled(),true);assert.equal(await preview.getAttribute('data-preview-state'),'starting');assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewAttaches.length),1);assert.equal(await p.evaluate(()=>[...window.uiHarness.previewProjections.keys()].at(-1)),lease);
    await p.getByRole('button',{name:'重新核对窗口',exact:true}).click();await p.getByText('窗口正在重新核对，暂保留上次确认的关联。',{exact:false}).waitFor({state:'hidden'});await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={...port.bindingFixture,state:'stale',errorCode:'target_changed',detail:'Controlled actual target invalidation'};for(const projection of port.previewProjections.values())projection.state='stopped';});await p.getByText('关联窗口已失效',{exact:true}).waitFor();assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isDisabled(),true);assert.equal(await preview.getAttribute('data-preview-state'),null);assert.equal(await p.evaluate(lease=>window.uiHarness.previewProjections.get(lease).state,lease),'stopped');assert.ok((await p.evaluate(()=>window.uiHarness.previewDetaches)).includes(lease));assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.filter(item=>item.action==='stop').length),0,'Session invalidation belongs to Rust, not React unmount');await p.close();
  });
  await check('RBC-28-same-session-window-rebind-isolates-late-unbind',async()=>{
    for(const fail of [false,true]){const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await openWindowManagement(p);await p.getByRole('button',{name:'解除窗口关联',exact:true}).waitFor();await p.evaluate(fail=>window.uiHarness.unbindResponses.push({holdKey:'same-session-unbind',...(fail?{error:'Controlled old-window unbind failure'}:{})}),fail);await openWindowManagement(p);await p.getByRole('button',{name:'解除窗口关联',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.unbindPending.has('same-session-unbind'));await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={state:'bound',sessionId:port.state.session.sessionId,targetIdentitySha256:'c'.repeat(64),windowTitle:'Same session replacement window',verifiedAtMs:'1792368005000',errorCode:null,detail:'Explicit same-session new complete window identity'};});await p.getByRole('region',{name:'运行窗口预览',exact:true}).getByText('Same session replacement window',{exact:true}).waitFor();const queries=await p.evaluate(()=>window.uiHarness.bindingQueries);await p.evaluate(()=>window.uiHarness.releaseUnbind('same-session-unbind'));await p.waitForFunction(()=>!document.querySelector('button[aria-busy=true]'));assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isEnabled(),true);assert.equal(await p.getByRole('button',{name:'解除窗口关联',exact:true}).count(),1);assert.equal(await p.evaluate(()=>window.uiHarness.bindingQueries),queries);assert.equal(await p.getByRole('alert').filter({hasText:'Controlled old-window unbind failure'}).count(),0);const request=await p.evaluate(()=>window.uiHarness.commands.find(item=>item.command.kind==='unbind_window').command.payload);assert.equal(request.expectedTargetIdentitySha256,'b'.repeat(64));await p.close();}
  });

  const applySqlProjection = async (p, scenario) => {
    const snapshot = sqlProjectionFixtures.get(scenario);
    await p.evaluate(snapshot => { const port = window.uiHarness; port.bindingFixture = { state:'unbound', sessionId:snapshot.session?.sessionId ?? null, targetIdentitySha256:null, windowTitle:null, verifiedAtMs:null, errorCode:null, detail:'Controlled unbound window; the reading snapshot comes from the real SQLite test producer.' }; port.publish(snapshot); }, snapshot);
    await p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    return snapshot;
  };
  const sentenceButtons = p => ['复制原文','重新翻译本句','分析本句文法'].map(name => p.getByRole('button',{name,exact:true}));
  const assertNoSentenceActions = async p => {
    const before = await p.evaluate(() => ({copies:window.uiHarness.copiedReadingUnits.length,retranslations:window.uiHarness.retranslations.length,learning:window.uiHarness.state.learningObject,windows:window.uiHarness.windows.length}));
    for (const button of sentenceButtons(p)) { assert.equal(await button.isDisabled(),true); await button.evaluate(node => node.click()); }
    assert.deepEqual(await p.evaluate(() => ({copies:window.uiHarness.copiedReadingUnits.length,retranslations:window.uiHarness.retranslations.length,learning:window.uiHarness.state.learningObject,windows:window.uiHarness.windows.length})),before);
  };
  const assertCurrentSentence = async (p, snapshot) => {
    const unit=snapshot.current; assert.ok(unit); const original=p.getByTestId('source-text'); await original.waitFor();
    assert.equal(await original.evaluate(node => {const clone=node.cloneNode(true);clone.querySelectorAll('rt,rp').forEach(n=>n.remove());return clone.textContent;}),unit.sourceText);
    for(const translation of unit.translations) assert.equal(await p.getByText(translation.correction?.text ?? translation.text,{exact:true}).isVisible(),true);
    for(const old of snapshot.history.units.filter(old=>old.sessionId!==unit.sessionId)) { assert.equal(await p.getByText(old.sourceText,{exact:true}).count(),0); for(const translation of old.translations)assert.equal(await p.getByText(translation.correction?.text??translation.text,{exact:true}).count(),0); }
    assert.equal(await p.getByText('等待游戏原文',{exact:true}).count(),0);
  };
  const assertSentenceActions = async (p, snapshot) => {
    const unit=snapshot.current; assert.ok(unit);
    for(const button of sentenceButtons(p))assert.equal(await button.isEnabled(),true);
    await p.getByRole('button',{name:'复制原文',exact:true}).click();assert.deepEqual(await p.evaluate(()=>window.uiHarness.copiedReadingUnits.at(-1)),{unitId:unit.unitId,textRevision:unit.textRevision,text:unit.sourceText});
    await p.getByRole('button',{name:'重新翻译本句',exact:true}).click();const request=await p.evaluate(()=>window.uiHarness.retranslations.at(-1));assert.equal(request.unitId,unit.unitId);assert.equal(request.textRevision,unit.textRevision);assert.ok(request.operationId);
    await p.getByRole('button',{name:'分析本句文法',exact:true}).click();assert.deepEqual(await p.evaluate(()=>({unitId:window.uiHarness.state.learningObject.unit.unitId,textRevision:window.uiHarness.state.learningObject.unit.textRevision,sessionId:window.uiHarness.state.learningObject.unit.sessionId,tokenIndex:window.uiHarness.state.learningObject.tokenIndex})),{unitId:unit.unitId,textRevision:unit.textRevision,sessionId:unit.sessionId,tokenIndex:null});
  };
  await check('RBC-29-new-hook-session-without-body-has-no-previous-reading-actions',async()=>{
    const p=await appPage('hud',560,290),snapshot=await applySqlProjection(p,'01-new-session-waiting');assert.equal(snapshot.current,null);assert.ok(snapshot.history.units.some(unit=>unit.sessionId!==snapshot.session.sessionId));await p.getByText('等待游戏原文',{exact:true}).waitFor();assert.equal(await p.getByTestId('source-text').count(),0);
    for(const unit of snapshot.history.units){assert.equal(await p.getByText(unit.sourceText,{exact:true}).count(),0);for(const t of unit.translations)assert.equal(await p.getByText(t.correction?.text??t.text,{exact:true}).count(),0);}await assertNoSentenceActions(p);await screenshot(p,'new-hook-session-waiting-no-old-sentence');await p.close();
  });
  await check('RBC-30-late-previous-session-history-cannot-become-current-reading',async()=>{
    const p=await appPage('hud',560,290),before=await applySqlProjection(p,'01-new-session-waiting'),late=await applySqlProjection(p,'02-old-session-late');assert.equal(late.session.sessionId,before.session.sessionId);assert.equal(late.current,null);assert.ok(late.commitRevision>before.commitRevision,'The actual SQLite late-callback fixture must advance its persisted revision');assert.ok(late.history.units.some(unit=>unit.sessionId!==late.session.sessionId));await p.getByText('等待游戏原文',{exact:true}).waitFor();assert.equal(await p.getByTestId('source-text').count(),0);for(const unit of late.history.units){assert.equal(await p.getByText(unit.sourceText,{exact:true}).count(),0);for(const t of unit.translations)assert.equal(await p.getByText(t.correction?.text??t.text,{exact:true}).count(),0);}await assertNoSentenceActions(p);await screenshot(p,'late-old-session-result-remains-waiting');await p.close();
  });
  await check('RBC-31-current-session-reading-arrival-keeps-action-object-identity',async()=>{
    const p=await appPage('hud',560,290);await applySqlProjection(p,'02-old-session-late');const snapshot=await applySqlProjection(p,'03-new-session-body');assert.equal(snapshot.current.sessionId,snapshot.session.sessionId);assert.ok(snapshot.current.translations.length);await assertCurrentSentence(p,snapshot);await assertSentenceActions(p,snapshot);await screenshot(p,'new-session-source-translation-and-actions');await p.close();
    const main=await appPage();await applySqlProjection(main,'03-new-session-body');await main.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(main,'正文历史');const overview=main.getByRole('article',{name:'当前原文与译文',exact:true});await overview.waitFor();assert.equal(await overview.getByTestId('source-text').evaluate(node=>{const clone=node.cloneNode(true);clone.querySelectorAll('rt,rp').forEach(n=>n.remove());return clone.textContent;}),snapshot.current.sourceText);for(const t of snapshot.current.translations)assert.equal(await overview.getByText(t.correction?.text??t.text,{exact:true}).isVisible(),true);await screenshot(main,'main-current-session-authoritative-reading');await main.close();
  });
  await check('RBC-32-stopped-session-last-reading-keeps-matching-actions',async()=>{
    const p=await appPage('hud',560,290);const active=await applySqlProjection(p,'03-new-session-body'),stopped=await applySqlProjection(p,'04-disconnected-last-sentence');assert.equal(stopped.session.state,'stopped');assert.equal(stopped.session.sessionId,active.session.sessionId);assert.equal(stopped.current.unitId,active.current.unitId);assert.equal(stopped.current.textRevision,active.current.textRevision);await assertCurrentSentence(p,stopped);await assertSentenceActions(p,stopped);assert.equal(await p.getByRole('button',{name:'暂停自动输入',exact:true}).isDisabled(),true);assert.equal(await p.getByRole('button',{name:'关闭自动翻译',exact:true}).isDisabled(),true);await screenshot(p,'disconnected-last-sentence-and-fixed-actions');await p.close();
  });
  // Controlled transport exercises the actual React panel; it is not SQLite/game evidence.
  const historyPage = async (count, completed = false) => {
    const initialize = (port, count, completed) => {
      const id = port.state.session.sessionId, base = structuredClone(port.state.current);
      const rows = Array.from({length:count}, (_,i) => ({kind:'body',sequence:String(i+1),sourceRefs:[{eventId:`history-raw-${i+1}`,byteStart:0,byteEnd:3}],unit:{...base,unitId:`history-unit-${i+1}`,sessionId:id,textRevision:1,sourceHash:String(i+1).padStart(64,'0'),sourceText:`原文 ${i+1}`,tokens:[],translations:completed?[{translationId:`history-translation-${i+1}`,taskId:`history-task-${i+1}`,text:`既有译文 ${i+1}`,providerLabel:'controlled persisted result',targetLanguage:'zh-CN',correction:null}]:[]}}));
      port.sessionRecordFixtures[`${id}:body`] = rows;
      const original = port.sessionRecords.bind(port);
      const control = {plans:[],pending:new Map(),receipts:[],active:new Map(),maxActive:new Map()};
      port.historyControl = control;
      port.sessionRecords = async query => {
        const scope=JSON.stringify([query.sessionId,query.dataset,query.filter]), active=(control.active.get(scope)??0)+1;
        control.active.set(scope,active); control.maxActive.set(scope,Math.max(active,control.maxActive.get(scope)??0));
        try {
          const result=await original(query);
          const at=control.plans.findIndex(plan=>(!plan.cursorOnly||Boolean(query.cursor))&&(!plan.latestOnly||query.cursor===null)&&(plan.offset===undefined||port.sessionRecordCursors.get(query.cursor)?.offset===plan.offset));
          const plan=at<0?null:control.plans.splice(at,1)[0];
          if(plan?.holdKey) await new Promise(resolve=>control.pending.set(plan.holdKey,()=>{control.pending.delete(plan.holdKey);resolve();}));
          if(plan?.typedError) throw structuredClone(plan.typedError);
          if(plan && 'error' in plan) throw new Error(plan.error);
          if(plan?.wrongScope) result.snapshot.snapshotId='controlled-wrong-snapshot';
          control.receipts.push({query:structuredClone(query),snapshot:structuredClone(result.snapshot),nextCursor:result.nextCursor,sequences:result.records.map(row=>row.sequence)});
          return result;
        } finally { control.active.set(scope,(control.active.get(scope)??1)-1); }
      };
    };
    const p = await appPage('main',1040,720,new Function('port', `(${initialize.toString()})(port, ${JSON.stringify(count)}, ${JSON.stringify(completed)});`));
    await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');
    await p.waitForFunction(expected=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===expected,Math.min(count,50));
    return p;
  };
  const historyRow = (p, sequence) => p.locator(`[data-record-kind="body"][data-record-sequence="${sequence}"]`);
  const mutateHistory = async (p, changes) => p.evaluate(changes=>{
    const port=window.uiHarness,rows=port.sessionRecordFixtures[`${port.state.session.sessionId}:body`];
    for(const change of changes){const row=rows.find(row=>row.sequence===String(change.sequence));if(change.translations)row.unit.translations=change.translations;else row.unit.translations=[{translationId:`history-translation-${change.sequence}`,taskId:`history-task-${change.sequence}`,text:change.text,providerLabel:'controlled persisted result',targetLanguage:'zh-CN',correction:change.correction??null}];}
  },changes);
  const historyState = p => p.getByRole('log',{name:'正文历史列表',exact:true}).evaluate(node=>({highWatermark:node.dataset.highWatermark,sequences:[...node.querySelectorAll('[data-record-sequence]')].map(row=>row.dataset.recordSequence)}));
  const visibleHistoryAnchor = p => p.getByRole('log',{name:'正文历史列表',exact:true}).evaluate(node=>{const top=node.getBoundingClientRect().top,row=[...node.querySelectorAll('[data-record-sequence]')].find(row=>row.getBoundingClientRect().bottom>top);return{sequence:row?.dataset.recordSequence,offset:row?row.getBoundingClientRect().top-top:0};});
  const loadHistoryOlder = async (p, expected) => {await p.getByRole('button',{name:'加载较早记录',exact:true}).click();await p.waitForFunction(expected=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===expected,expected);};
  const pauseHistory = async p => {const follow=p.getByRole('button',{name:'跟随新内容',exact:true});if(await follow.count())await follow.click();};
  const holdHistoryRefresh = async (p,key) => {await p.evaluate(key=>window.uiHarness.historyControl.plans.push({cursorOnly:true,offset:0,holdKey:key}),key);await p.waitForFunction(key=>window.uiHarness.historyControl.pending.has(key),key);};
  const releaseHistory = async (p,key) => p.evaluate(key=>window.uiHarness.historyControl.pending.get(key)(),key);
  await check('RBC-33-body-late-translation-refreshes-with-zero-newer-and-frozen-revision',async()=>{
    const p=await historyPage(1),before=await historyState(p),initial=await p.evaluate(()=>window.uiHarness.historyControl.receipts.find(receipt=>receipt.query.dataset==='body'));
    await historyRow(p,1).getByText('尚无译文，原文保持可读。',{exact:true}).waitFor();
    await mutateHistory(p,[{sequence:1,text:'同一句晚到的权威译文'}]);
    await historyRow(p,1).getByText('同一句晚到的权威译文',{exact:false}).waitFor();
    assert.deepEqual(await historyState(p),before);const receipts=await p.evaluate(()=>window.uiHarness.historyControl.receipts.filter(receipt=>receipt.query.dataset==='body'));
    assert.ok(receipts.length>=2);assert.equal(receipts.every(receipt=>receipt.snapshot.snapshotId===initial.snapshot.snapshotId&&receipt.snapshot.highWatermark===initial.snapshot.highWatermark&&receipt.snapshot.databaseRevision===initial.snapshot.databaseRevision&&receipt.snapshot.newerCount==='0'),true);
    assert.equal(receipts.filter(receipt=>receipt.query.dataset==='body'&&receipt.query.cursor===null).length,1);assert.equal(await historyRow(p,1).getByText('尚无译文，原文保持可读。',{exact:true}).count(),0);await screenshot(p,'body-late-translation-zero-newer');await p.close();
  });
  await check('RBC-34-loaded-body-pages-refresh-content-and-preserve-cursor-and-anchor',async()=>{
    const p=await historyPage(160);await loadHistoryOlder(p,100);await loadHistoryOlder(p,150);await pauseHistory(p);
    const tail=await p.evaluate(()=>window.uiHarness.historyControl.receipts.at(-1).nextCursor),before=await historyState(p),list=p.getByRole('log',{name:'正文历史列表',exact:true});
    await list.evaluate(node=>{const row=node.querySelector('[data-record-sequence="80"]');node.scrollTop+=row.getBoundingClientRect().top-node.getBoundingClientRect().top+17;});
    const anchor=await visibleHistoryAnchor(p);assert.equal(anchor.sequence,'80');
    await mutateHistory(p,[{sequence:15,text:'较早页迟到译文。'.repeat(60)},{sequence:160,text:'首个已加载页的迟到译文'},{sequence:65,text:'旧译文',correction:{revision:2,text:'中间页的人工修正'}}]);
    await historyRow(p,15).getByText('较早页迟到译文。'.repeat(60),{exact:false}).waitFor();await historyRow(p,160).getByText('首个已加载页的迟到译文',{exact:false}).waitFor();await historyRow(p,65).getByText('中间页的人工修正',{exact:false}).waitFor();
    assert.deepEqual(await historyState(p),before);const after=await visibleHistoryAnchor(p);assert.equal(after.sequence,anchor.sequence);assert.ok(Math.abs(after.offset-anchor.offset)<=1,JSON.stringify({anchor,after}));
    assert.equal(new Set(before.sequences).size,150);await loadHistoryOlder(p,160);const last=await p.evaluate(()=>window.uiHarness.historyControl.receipts.at(-1));assert.equal(last.query.cursor,tail);assert.deepEqual((await historyState(p)).sequences,Array.from({length:160},(_,i)=>String(i+1)));await screenshot(p,'body-loaded-pages-late-content');await p.close();
  });
  await check('RBC-35-paused-body-refresh-keeps-high-watermark-while-new-count-arrives',async()=>{
    const p=await historyPage(3);await pauseHistory(p);const before=await historyState(p);
    await p.evaluate(()=>{const port=window.uiHarness,rows=port.sessionRecordFixtures[`${port.state.session.sessionId}:body`],last=structuredClone(rows.at(-1));rows.push({...last,sequence:'4',unit:{...last.unit,unitId:'history-unit-4',sourceText:'原文 4',sourceHash:'4'.padStart(64,'0')}});});
    await mutateHistory(p,[{sequence:2,text:'固定范围内已完成译文'}]);await p.getByRole('button',{name:'回到最新 · 1 条新内容',exact:true}).waitFor();await historyRow(p,2).getByText('固定范围内已完成译文',{exact:false}).waitFor();
    assert.deepEqual(await historyState(p),before);assert.equal(await historyRow(p,4).count(),0);await p.getByRole('button',{name:'回到最新 · 1 条新内容',exact:true}).click();await historyRow(p,4).waitFor();assert.equal((await historyState(p)).highWatermark,'4');assert.equal(await p.getByRole('button',{name:'跟随新内容',exact:true}).count(),1);await p.close();
  });
  await check('RBC-36-body-stale-scope-replies-and-overlapping-page-reads-are-isolated',async()=>{
    const queued=await historyPage(160);await pauseHistory(queued);await holdHistoryRefresh(queued,'queue-older');
    const beforeQueued=await queued.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await queued.getByRole('button',{name:'加载较早记录',exact:true}).click();assert.equal(await queued.evaluate(()=>window.uiHarness.sessionRecordQueries.length),beforeQueued);await releaseHistory(queued,'queue-older');await queued.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===100);assert.equal(await queued.evaluate(()=>Math.max(...window.uiHarness.historyControl.maxActive.values())),1);
    await holdHistoryRefresh(queued,'queue-priority');const priorityStart=await queued.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await queued.getByRole('button',{name:'加载较早记录',exact:true}).click();
    await queued.evaluate(()=>{const port=window.uiHarness,rows=port.sessionRecordFixtures[`${port.state.session.sessionId}:body`],last=structuredClone(rows.at(-1));rows.push({...last,sequence:'161',unit:{...last.unit,unitId:'history-unit-161',sourceHash:'161'.padStart(64,'0'),sourceText:'原文 161'}});});
    await queued.getByRole('button',{name:/^回到最新/}).click();assert.equal(await queued.evaluate(()=>window.uiHarness.sessionRecordQueries.length),priorityStart);await releaseHistory(queued,'queue-priority');await historyRow(queued,161).waitFor();assert.equal(await queued.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);assert.equal((await historyState(queued)).highWatermark,'161');assert.equal(await queued.getByRole('button',{name:'跟随新内容',exact:true}).count(),1);
    const drained=await queued.evaluate(start=>window.uiHarness.sessionRecordQueries.slice(start).map(query=>({cursor:query.cursor,offset:query.cursor?window.uiHarness.sessionRecordCursors.get(query.cursor).offset:null})),priorityStart);assert.equal(drained.filter(query=>query.cursor===null).length,1);assert.equal(drained.some(query=>query.offset===100),false);assert.equal(await queued.evaluate(()=>Math.max(...window.uiHarness.historyControl.maxActive.values())),1);await queued.close();
    const p=await historyPage(160);await pauseHistory(p);await holdHistoryRefresh(p,'held-poll');const count=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await p.getByRole('button',{name:'加载较早记录',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length),count);assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);assert.equal(await p.evaluate(()=>Math.max(...window.uiHarness.historyControl.maxActive.values())),1);
    const oldEmptyScopeQueries=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.dataset==='body'&&!query.filter.search).length);await p.getByRole('textbox',{name:'搜索会话原文',exact:true}).fill('原文 140');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===1);assert.equal(await historyRow(p,140).count(),1);await releaseHistory(p,'held-poll');await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),1);assert.equal(await historyRow(p,140).count(),1);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.dataset==='body'&&!query.filter.search).length),oldEmptyScopeQueries);
    await holdHistoryRefresh(p,'old-session-poll');await p.getByRole('button',{name:/^回到最新/}).click();const oldSessionQueries=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId==='ui-fixture-session').length);await p.evaluate(()=>{const port=window.uiHarness,id='replacement-history-session',row=structuredClone(port.sessionRecordFixtures[`${port.state.session.sessionId}:body`][0]);row.unit={...row.unit,unitId:'replacement-unit',sessionId:id,sourceText:'新会话原文'};row.sequence='901';port.sessionRecordFixtures[`${id}:body`]=[row];port.bindingFixture={...port.bindingFixture,sessionId:id};port.publish({session:{...port.state.session,sessionId:id}});});await openSessionDataset(p,'正文历史');await historyRow(p,901).waitFor();await releaseHistory(p,'old-session-poll');await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await historyRow(p,901).count(),1);assert.equal(await historyRow(p,140).count(),0);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId==='ui-fixture-session').length),oldSessionQueries);
    await pauseHistory(p);await holdHistoryRefresh(p,'old-dataset-poll');await p.getByRole('button',{name:/^回到最新/}).click();const oldDatasetQueries=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId==='replacement-history-session'&&query.dataset==='body').length);await openSessionDataset(p,'全部提取');await p.getByRole('log',{name:'全量提取文本列表',exact:true}).waitFor();const extraction=p.locator('[data-session-dataset="extraction"]'),extractionBefore=await extraction.locator('[data-record-sequence]').allTextContents();await releaseHistory(p,'old-dataset-poll');await p.waitForFunction(()=>[...window.uiHarness.historyControl.active.values()].every(count=>count===0));assert.equal(await historyRow(p,901).count(),1,'Closing retains the mounted body range');assert.equal(await p.locator('[data-session-dataset="body"] [role="log"]').count(),1);assert.equal(await p.getByRole('log',{name:'正文历史列表',exact:true}).count(),0,'The retained body remains collapsed');assert.deepEqual(await extraction.locator('[data-record-sequence]').allTextContents(),extractionBefore,'Body replies cannot change extraction');assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId==='replacement-history-session'&&query.dataset==='body').length),oldDatasetQueries+1,'The queued explicit latest intent completes in its original panel');const settledQueries=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId==='replacement-history-session'&&query.dataset==='body').length);await p.waitForTimeout(1200);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.sessionId==='replacement-history-session'&&query.dataset==='body').length),settledQueries,'Collapsed body stops background polling');await p.close();
    const wrong=await historyPage(2);await pauseHistory(wrong);await wrong.evaluate(()=>window.uiHarness.historyControl.plans.push({cursorOnly:true,wrongScope:true}));const before=await historyState(wrong);await wrong.getByRole('alert').filter({hasText:'刷新返回了不同记录或查看范围'}).waitFor();assert.deepEqual(await historyState(wrong),before);await wrong.getByRole('button',{name:'重试',exact:true}).click();await wrong.getByRole('alert').waitFor({state:'hidden'});assert.deepEqual(await historyState(wrong),before);await wrong.close();
  });
  await check('RBC-37-body-refresh-failure-keeps-all-pages-and-retry-reconciles-content',async()=>{
    const p=await historyPage(160,true);await loadHistoryOlder(p,100);await pauseHistory(p);const before=await historyState(p);
    await p.evaluate(()=>window.uiHarness.historyControl.plans.push({cursorOnly:true,offset:50,error:'Controlled loaded older-page refresh failure'}));await mutateHistory(p,[{sequence:160,text:'刷新成功后首个已加载页的新译文'},{sequence:65,text:'刷新成功后较早已加载页的新译文'}]);
    await p.getByRole('alert').filter({hasText:'Controlled loaded older-page refresh failure'}).waitFor();assert.deepEqual(await historyState(p),before);await historyRow(p,160).getByText('既有译文 160',{exact:false}).waitFor();await historyRow(p,65).getByText('既有译文 65',{exact:false}).waitFor();assert.equal(await historyRow(p,160).getByText('刷新成功后首个已加载页的新译文',{exact:false}).count(),0);
    await p.getByRole('button',{name:'重试',exact:true}).click();await p.getByRole('alert').waitFor({state:'hidden'});await historyRow(p,160).getByText('刷新成功后首个已加载页的新译文',{exact:false}).waitFor();await historyRow(p,65).getByText('刷新成功后较早已加载页的新译文',{exact:false}).waitFor();assert.deepEqual(await historyState(p),before);assert.equal(await p.evaluate(()=>window.uiHarness.historyControl.receipts.filter(receipt=>receipt.query.dataset==='body'&&receipt.query.cursor===null).length),1);await p.close();
  });
  await check('RBC-38-body-retranslation-and-correction-refresh-only-matching-history-object',async()=>{
    const p=await historyPage(2,true);await pauseHistory(p);const before=await historyState(p);await p.evaluate(()=>{const port=window.uiHarness,row=port.sessionRecordFixtures[`${port.state.session.sessionId}:body`][0];port.frozenStudyBefore=structuredClone(row.unit);port.publish({learningObject:{unit:structuredClone(row.unit),tokenIndex:null}});});
    for(const state of ['running','cancelled','failed']){await p.evaluate(state=>{const port=window.uiHarness;port.publish({tasks:[{taskId:'controlled-retranslation',unitId:'history-unit-1',textRevision:1,purpose:'translation',status:state,errorCode:state==='failed'?'controlled_failure':null,partialText:null,providerLabel:'controlled retranslation',attemptId:null}]});},state);const reads=await p.evaluate(()=>window.uiHarness.historyControl.receipts.length);await p.waitForFunction(reads=>window.uiHarness.historyControl.receipts.length>reads,reads);await historyRow(p,1).getByText('既有译文 1',{exact:false}).waitFor();assert.equal(await historyRow(p,1).getByText('尚无译文，原文保持可读。',{exact:true}).count(),0);}
    await mutateHistory(p,[{sequence:1,translations:[{translationId:'history-translation-1',taskId:'history-task-1',text:'既有译文 1',providerLabel:'controlled persisted result',targetLanguage:'zh-CN',correction:null},{translationId:'completed-retranslation-1',taskId:'completed-retranslation-task-1',text:'重新翻译实际完成结果',providerLabel:'controlled completed retranslation',targetLanguage:'zh-CN',correction:null}]}]);await historyRow(p,1).getByText('重新翻译实际完成结果',{exact:false}).waitFor();await historyRow(p,1).getByText('既有译文 1',{exact:false}).waitFor();
    await mutateHistory(p,[{sequence:1,translations:[{translationId:'history-translation-1',taskId:'history-task-1',text:'既有译文 1',providerLabel:'controlled persisted result',targetLanguage:'zh-CN',correction:{revision:2,text:'第一句保存后的人工修正'}},{translationId:'completed-retranslation-1',taskId:'completed-retranslation-task-1',text:'重新翻译实际完成结果',providerLabel:'controlled completed retranslation',targetLanguage:'zh-CN',correction:null}]}]);await historyRow(p,1).getByText('第一句保存后的人工修正',{exact:false}).waitFor();assert.equal(await historyRow(p,1).getByText('既有译文 1',{exact:false}).count(),0);await historyRow(p,2).getByText('既有译文 2',{exact:false}).waitFor();assert.equal(await historyRow(p,2).getByText('第一句保存后的人工修正',{exact:false}).count(),0);assert.deepEqual(await historyState(p),before);assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.learningObject.unit),await p.evaluate(()=>window.uiHarness.frozenStudyBefore));await screenshot(p,'body-retranslation-and-correction-matching-identity');await p.close();
    // Deterministic transport vectors exercise rendering only. The real
    // Rust/SQLite history producer below owns the actual head/membership proof.
    const waitReadingQuery = async (p, target) => {
      await p.waitForFunction(target => window.uiHarness.readingProjectionQueries.some(query => query.unitId === target.unitId && query.textRevision === target.textRevision && query.commitRevision >= target.commitRevision), target);
      await p.waitForFunction(() => [...window.uiHarness.readingProjectionActive.values()].every(count => count === 0));
      await p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    };
    for (const role of ['hud', 'session']) {
      const p = await appPage(role === 'hud' ? 'hud' : 'main', role === 'hud' ? 560 : 1040, 720);
      if (role === 'session') { await p.getByRole('button', { name: '当前会话', exact: true }).click(); await openSessionDataset(p,'正文历史'); }
      const surface = role === 'hud' ? p.locator('[data-ui-scroll="reading"]') : p.getByRole('article', { name: '当前原文与译文', exact: true });
      const project = async phase => {
        const target = await p.evaluate(phase => {
          const port = window.uiHarness, base = port.state.current;
          if (!base || base.sessionId !== port.state.session.sessionId) throw Error('Controlled recovery must retain its exact active unit');
          const task = (taskId, status, errorCode = null) => ({ taskId, unitId: base.unitId, textRevision: base.textRevision, purpose: 'translation', status, errorCode, partialText: null, providerLabel: '同名受控服务', attemptId: `attempt-${taskId}` });
          const current = task('current-a', phase === 'before' || phase === 'failed' ? 'failed' : phase === 'running' ? 'running' : phase === 'cancelled' ? 'cancelled' : 'completed', phase === 'before' ? 'connection_failed' : phase === 'failed' ? 'timeout' : null);
          const translations = ['before', 'running'].includes(phase) ? [] : [{ translationId: 'actual-new-result', taskId: 'current-a', text: '新的翻译结果保留。', providerLabel: '同名受控服务', targetLanguage: 'zh-CN', correction: null }];
          if (['all-completed', 'mixed-unrequested', 'unrequested'].includes(phase)) translations.push({ translationId: 'retained-engine-b-result', taskId: 'independent-engine-b', text: '另一引擎已有译文保留。', providerLabel: '同名受控服务', targetLanguage: 'zh-CN', correction: null });
          const unit = { ...base, translations }, commitRevision = port.state.commitRevision + 1;
          const status = phase === 'unrequested' ? { kind: 'unrequested' } : phase === 'legacy' ? { kind: 'unordered', summary: { headCount: '2', queuedCount: '0', runningCount: '0', completedCount: '1', failedCount: '1', cancelledCount: '0', invalidatedCount: '0' }, activeTask: null } : phase === 'corrupt' ? { kind: 'unavailable', error: { code: 'invalid_translation_status_lineage', stage: 'storage', message: '该引擎的历史请求归属记录无效，已隔离。', recoverable: true } } : { kind: 'current', task: current };
          const engines = [{ scopeKey: 'global', bindingId: 'controlled-binding', engineId: 'engine-a', providerLabel: '同名受控服务', status }];
          if (['multi', 'failed', 'cancelled', 'corrupt'].includes(phase)) engines.push({ scopeKey: 'global', bindingId: 'controlled-binding', engineId: 'engine-b', providerLabel: '同名受控服务', status: { kind: 'current', task: task('independent-engine-b', 'failed', 'service_unavailable') } });
          if (['all-completed', 'mixed-unrequested', 'unrequested'].includes(phase)) engines.push({ scopeKey: 'global', bindingId: 'controlled-binding', engineId: 'engine-b', providerLabel: '同名受控服务', status: phase === 'all-completed' ? { kind: 'current', task: task('independent-engine-b', 'completed') } : { kind: 'unrequested' } });
          const projection = { profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit, translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines } };
          port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = structuredClone(projection);
          // Deliberately retain an obsolete failure in the global task history.
          const tasks = [task('old-failed-a', 'failed', 'connection_failed'), { ...task('different-unit', 'failed', 'wrong_unit'), unitId: 'different-unit' }, { ...task('different-revision', 'failed', 'wrong_revision'), textRevision: unit.textRevision + 1 }];
          if (phase === 'running') tasks.push({ ...current, partialText: '新的请求片段' });
          port.publish({ commitRevision, current: unit, history: { ...port.state.history, units: [unit] }, tasks });
          return { unitId: unit.unitId, textRevision: unit.textRevision, commitRevision };
        }, phase);
        await waitReadingQuery(p, target);
      };
      const failed = code => surface.getByText(`同名受控服务 · 翻译失败：${code}`, { exact: true });
      await project('before'); assert.equal(await failed('connection_failed').isVisible(), true);
      await project('running'); assert.equal(await surface.getByText('同名受控服务 · 翻译中', { exact: true }).isVisible(), true); assert.equal(await failed('connection_failed').count(), 0); assert.equal(await surface.getByText('新的请求片段', { exact: true }).isVisible(), true);
      await project('success'); assert.equal(await surface.getByText('新的翻译结果保留。', { exact: true }).isVisible(), true); assert.equal(await failed('connection_failed').count(), 0); assert.equal(await surface.getByText('同名受控服务 · 翻译中', { exact: true }).count(), 0);
      await screenshot(p, `translation-current-success-${role}`);
      await project('all-completed');
      if (role === 'session') assert.equal(await surface.locator('footer > span').last().textContent(), '已完成', 'All current engine members completed must positively show completion');
      for (const phase of ['mixed-unrequested', 'unrequested']) {
        await project(phase);
        if (role === 'session') assert.equal(await surface.locator('footer > span').last().textContent(), '当前原句', 'An unrequested current member cannot be omitted from completion');
        for (const text of ['新的翻译结果保留。', '另一引擎已有译文保留。']) assert.equal(await surface.getByText(text, { exact: true }).isVisible(), true);
      }
      await project('multi'); assert.equal(await failed('service_unavailable').isVisible(), true); assert.equal(await failed('connection_failed').count(), 0);
      await project('failed'); assert.equal(await failed('timeout').isVisible(), true); assert.equal(await failed('service_unavailable').isVisible(), true); assert.equal(await surface.getByText('新的翻译结果保留。', { exact: true }).isVisible(), true);
      await project('cancelled'); assert.equal(await surface.getByText('同名受控服务 · 翻译已取消', { exact: true }).isVisible(), true); assert.equal(await failed('timeout').count(), 0); assert.equal(await failed('service_unavailable').isVisible(), true);
      await project('legacy'); assert.equal(await surface.getByText(/请求归属未定，共 2 次请求，含 1 次失败请求/).isVisible(), true); assert.equal(await surface.getByText('新的翻译结果保留。', { exact: true }).isVisible(), true);
      await project('corrupt'); assert.equal(await surface.getByText(/翻译状态无法核验：该引擎的历史请求归属记录无效，已隔离。/).isVisible(), true); assert.equal(await failed('service_unavailable').isVisible(), true);
      assert.equal(await failed('wrong_unit').count(), 0); assert.equal(await failed('wrong_revision').count(), 0);
      await screenshot(p, `translation-lane-isolated-${role}`); await p.close();
    }
    // These objects/pages/states came from the actual Runtime + SQLite producer.
    // DesignPort only forwards each exact query/retranslation/cancellation reply.
    const actual = actualReadingHistory, selected = actual.selectedUnit;
    const sameUnit = unit => unit?.unitId === selected.unitId && unit.textRevision === selected.textRevision;
    assert.equal(sameUnit(actual.initialSnapshot.current), false);
    assert.equal(actual.initialSnapshot.history.units.some(sameUnit), false);
    assert.equal(actual.firstPage.units.length, 50);
    assert.equal(actual.firstPage.units.some(sameUnit), false);
    assert.equal(actual.nextPage.units.some(sameUnit), true);
    assert.equal(actual.nextQuery.cursor, actual.firstPage.nextCursor);
    assert.equal(typeof actual.nextQuery.cursor, 'string'); assert.ok(actual.nextQuery.cursor.length > 0);
    assert.deepEqual(actual.firstQuery, { filter: { gameId: null, sourceKind: null, sessionId: null }, cursor: null, limit: 50 });
    const actualStages = new Map(actual.stages.map(stage => [stage.name, stage]));
    for (const name of ['hidden_failed', 'new_running', 'hidden_cancelled', 'recovered', 'corrupt_lane']) assert.ok(actualStages.has(name), `Missing actual stage ${name}`);
    const head = (stage, status) => stage.projection.translationState.engines.find(engine => engine.status.kind === 'current' && engine.status.task.status === status)?.status.task;
    for (const [name, status] of [['hidden_failed', 'failed'], ['hidden_cancelled', 'cancelled'], ['recovered', 'completed']]) {
      const stage = actualStages.get(name), task = head(stage, status);
      assert.ok(task, `Actual ${name} must supply its full-DB head`);
      assert.equal(stage.snapshot.tasks.some(visible => visible.taskId === task.taskId), false, `Actual ${status} must remain outside the bounded historical tasks`);
      assert.ok(stage.snapshot.tasks.some(task => sameUnit(task) && task.status === 'completed'), 'The old visible success must remain in bounded history');
    }
    const actualPage = await appPage();
    try {
      await actualPage.evaluate(actual => {
        const port = window.uiHarness, key = unit => JSON.stringify([unit.unitId, unit.textRevision]);
        for (const [query, page] of [[actual.firstQuery, actual.firstPage], [actual.nextQuery, actual.nextPage]]) {
          port.historyQueryFixtures[JSON.stringify([query.filter.gameId, query.filter.sourceKind, query.filter.sessionId, query.cursor, query.limit])] = structuredClone(page);
          for (const unit of page.units) port.readingUnitFixtures[key(unit)] = structuredClone(unit);
        }
        port.readingUnitFixtures[key(actual.selectedUnit)] = structuredClone(actual.selectedUnit);
        port.readingProjectionFixtures[key(actual.selectedUnit)] = structuredClone(actual.firstProjection);
        port.publish(actual.initialSnapshot);
      }, actual);
      await actualPage.getByRole('button', { name: '历史记录', exact: true }).click();
      const records = actualPage.getByRole('complementary', { name: '历史筛选与记录', exact: true });
      assert.equal(await records.getByRole('button').filter({ hasText: selected.sourceText }).count(), 0);
      await records.getByRole('button', { name: '加载更早记录', exact: true }).click();
      await records.getByRole('button').filter({ hasText: selected.sourceText }).click();
      await waitReadingQuery(actualPage, { unitId: selected.unitId, textRevision: selected.textRevision, commitRevision: actual.firstProjection.commitRevision });
      assert.ok(await actualPage.evaluate(selected => window.uiHarness.readingUnitQueries.some(query => query.unitId === selected.unitId && query.textRevision === selected.textRevision), selected));
      assert.deepEqual(await actualPage.evaluate(cursor => window.uiHarness.historyQueries.find(query => query.cursor === cursor), actual.nextQuery.cursor), actual.nextQuery);
      const detail = records.locator('..').locator(':scope > section');
      const originalTranslations = selected.translations.map(translation => translation.correction?.text ?? translation.text);
      for (const text of originalTranslations) await detail.getByText(text, { exact: true }).waitFor();
      await detail.getByRole('button', { name: '精读本句', exact: true }).click();
      const frozen = await actualPage.evaluate(() => structuredClone(window.uiHarness.state.learningObject));
      assert.deepEqual(frozen.unit, selected);
      const forwardActual = async name => {
        const stage = actualStages.get(name);
        await actualPage.evaluate(stage => {
          const port = window.uiHarness, key = JSON.stringify([stage.projection.unit.unitId, stage.projection.unit.textRevision]);
          port.readingUnitFixtures[key] = structuredClone(stage.projection.unit);
          port.readingProjectionFixtures[key] = structuredClone(stage.projection);
          port.publish(stage.snapshot);
        }, stage);
        await waitReadingQuery(actualPage, { unitId: selected.unitId, textRevision: selected.textRevision, commitRevision: stage.projection.commitRevision });
      };
      await forwardActual('hidden_failed');
      const actualFailure = head(actualStages.get('hidden_failed'), 'failed');
      await detail.getByText(`${actualFailure.providerLabel} · 翻译失败：${actualFailure.errorCode}`, { exact: true }).waitFor();
      for (const text of originalTranslations) await detail.getByText(text, { exact: true }).waitFor();
      await screenshot(actualPage, 'actual-history-later-page-hidden-failed');
      await actualPage.evaluate(({ selected, running, cancelled, recovered }) => {
        const port = window.uiHarness;
        for (const stage of [running, recovered]) port.readingProjectionRetranslations.push({ unitId: selected.unitId, textRevision: selected.textRevision, snapshot: stage.snapshot, projection: stage.projection, receipts: stage.receipts });
        port.readingProjectionCancellations.push({ taskId: cancelled.cancelTaskId, snapshot: cancelled.snapshot, projection: cancelled.projection, receipt: cancelled.cancelReceipt });
      }, { selected, running: actualStages.get('new_running'), cancelled: actualStages.get('hidden_cancelled'), recovered: actualStages.get('recovered') });
      await detail.getByRole('button', { name: '使用当前配置重译', exact: true }).click();
      await waitReadingQuery(actualPage, { unitId: selected.unitId, textRevision: selected.textRevision, commitRevision: actualStages.get('new_running').projection.commitRevision });
      await detail.getByRole('button', { name: '取消', exact: true }).click();
      await waitReadingQuery(actualPage, { unitId: selected.unitId, textRevision: selected.textRevision, commitRevision: actualStages.get('hidden_cancelled').projection.commitRevision });
      const cancelled = head(actualStages.get('hidden_cancelled'), 'cancelled');
      await detail.getByText(`${cancelled.providerLabel} · 翻译已取消`, { exact: true }).waitFor();
      assert.equal(await detail.getByText(`${actualFailure.providerLabel} · 翻译失败：${actualFailure.errorCode}`, { exact: true }).count(), 0);
      await detail.getByRole('button', { name: '使用当前配置重译', exact: true }).click();
      await waitReadingQuery(actualPage, { unitId: selected.unitId, textRevision: selected.textRevision, commitRevision: actualStages.get('recovered').projection.commitRevision });
      const recoveredUnit = actualStages.get('recovered').projection.unit;
      for (const translation of recoveredUnit.translations) await detail.getByText(translation.correction?.text ?? translation.text, { exact: true }).waitFor();
      assert.equal(await detail.getByText(`${actualFailure.providerLabel} · 翻译失败：${actualFailure.errorCode}`, { exact: true }).count(), 0);
      assert.equal(await detail.getByText(`${cancelled.providerLabel} · 翻译已取消`, { exact: true }).count(), 0);
      await screenshot(actualPage, 'actual-history-later-page-recovered');
      await forwardActual('corrupt_lane');
      for (const engine of actualStages.get('corrupt_lane').projection.translationState.engines) {
        if (engine.status.kind === 'unavailable') await detail.getByText(`${engine.providerLabel} · 翻译状态无法核验：${engine.status.error.message}`, { exact: true }).waitFor();
        if (engine.status.kind === 'current' && engine.status.task.status === 'failed') await detail.getByText(`${engine.status.task.providerLabel} · 翻译失败：${engine.status.task.errorCode}`, { exact: true }).waitFor();
      }
      await actualPage.evaluate(({ selected, error }) => window.uiHarness.readingProjectionRetranslations.push({ unitId: selected.unitId, textRevision: selected.textRevision, error: error.message }), { selected, error: actual.rejectedAdmission });
      await detail.getByRole('button', { name: '使用当前配置重译', exact: true }).click();
      await detail.getByText(actual.rejectedAdmission.message, { exact: true }).waitFor();
      assert.deepEqual(await actualPage.evaluate(() => window.uiHarness.state.learningObject), frozen, 'Live projections must not mutate the frozen reading object');
      assert.deepEqual(await actualPage.evaluate(() => window.uiHarness.retranslations.map(value => [value.unitId, value.textRevision])), Array.from({ length: 3 }, () => [selected.unitId, selected.textRevision]));
      assert.equal(await actualPage.evaluate(selected => window.uiHarness.state.history.units.some(unit => unit.unitId === selected.unitId && unit.textRevision === selected.textRevision) || window.uiHarness.state.current?.unitId === selected.unitId, selected), false, 'The old object must remain outside the default snapshot');
      await screenshot(actualPage, 'actual-history-later-page-corrupt-lane');
    } finally { await actualPage.close(); }

    // Fault plans exercise the production hook, while these explicit transport
    // DTOs make no claim to be additional SQLite/product-state evidence.
    for (const rejectOld of [false, true]) {
      const ownerPage = await appPage('hud', 560, 720);
      try {
        const installOwner = async (name, holdKey, error = null) => ownerPage.evaluate(({ name, holdKey, error }) => {
          const port = window.uiHarness, base = port.state.current;
          const unit = { ...base, unitId: `controlled-owner-${name}`, sourceHash: name === 'A' ? '1'.repeat(64) : '2'.repeat(64), translations: [] };
          const commitRevision = port.state.commitRevision + 1;
          const task = { taskId: `controlled-task-${name}`, unitId: unit.unitId, textRevision: unit.textRevision, purpose: 'translation', status: 'failed', errorCode: `owner_${name}`, partialText: null, providerLabel: '受控 owner', attemptId: `attempt-${name}` };
          const projection = { profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit, translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines: [{ scopeKey: 'global', bindingId: 'controlled-owner-binding', engineId: 'controlled-owner-engine', providerLabel: task.providerLabel, status: { kind: 'current', task } }] } };
          port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = structuredClone(projection);
          port.readingProjectionPlans.push({ unitId: unit.unitId, value: projection, holdKey, error });
          port.publish({ commitRevision, current: unit });
          return { unitId: unit.unitId, textRevision: unit.textRevision, commitRevision };
        }, { name, holdKey, error });
        await installOwner('A', 'old-owner', rejectOld ? 'Controlled late old rejection' : null);
        await ownerPage.waitForFunction(() => window.uiHarness.readingProjectionPending.has('old-owner'));
        const newOwner = await installOwner('B', 'new-owner');
        await ownerPage.waitForFunction(() => window.uiHarness.readingProjectionPending.has('new-owner'));
        assert.equal(await ownerPage.evaluate(() => window.uiHarness.readingProjectionPending.has('old-owner')), true, 'A new object must read while the old owner remains unsettled');
        const beforeLate = await ownerPage.evaluate(() => window.uiHarness.readingProjectionQueries.length);
        await ownerPage.evaluate(() => window.uiHarness.releaseReadingProjection('old-owner'));
        await ownerPage.waitForTimeout(30);
        assert.equal(await ownerPage.evaluate(() => window.uiHarness.readingProjectionQueries.length), beforeLate, 'Old resolve/reject/finally cannot clear or reschedule the new pending read');
        assert.equal(await ownerPage.evaluate(() => window.uiHarness.readingProjectionPending.has('new-owner')), true);
        await ownerPage.evaluate(() => window.uiHarness.releaseReadingProjection('new-owner'));
        await waitReadingQuery(ownerPage, newOwner);
        await ownerPage.getByText('受控 owner · 翻译失败：owner_B', { exact: true }).waitFor();
        assert.equal(await ownerPage.getByText(/owner_A|Controlled late old rejection/).count(), 0);

        const firstDirty = await ownerPage.evaluate(() => {
          const port = window.uiHarness, key = JSON.stringify([port.state.current.unitId, port.state.current.textRevision]);
          const projection = structuredClone(port.readingProjectionFixtures[key]); projection.commitRevision = port.state.commitRevision + 1;
          port.readingProjectionPlans.push({ unitId: projection.unit.unitId, value: projection, holdKey: 'dirty-first' });
          port.publish({ commitRevision: projection.commitRevision }); return projection.commitRevision;
        });
        await ownerPage.waitForFunction(() => window.uiHarness.readingProjectionPending.has('dirty-first'));
        const newest = await ownerPage.evaluate(() => {
          const port = window.uiHarness, key = JSON.stringify([port.state.current.unitId, port.state.current.textRevision]);
          for (let index = 0; index < 8; index++) { const commitRevision = port.state.commitRevision + 1; port.readingProjectionFixtures[key].commitRevision = commitRevision; port.publish({ commitRevision }); }
          port.readingProjectionPlans.push({ unitId: port.state.current.unitId, value: structuredClone(port.readingProjectionFixtures[key]), holdKey: 'dirty-newest' });
          port.releaseReadingProjection('dirty-first'); return port.state.commitRevision;
        });
        await ownerPage.waitForFunction(() => window.uiHarness.readingProjectionPending.has('dirty-newest'));
        await ownerPage.getByText('翻译状态更新中，以下保留上次读取结果。', { exact: true }).waitFor();
        await ownerPage.getByText('上次读取 · 受控 owner · 翻译失败：owner_B', { exact: true }).waitFor();
        const dirtyReads = await ownerPage.evaluate(({ unitId, firstDirty }) => window.uiHarness.readingProjectionQueries.filter(query => query.unitId === unitId && query.commitRevision >= firstDirty), { unitId: newOwner.unitId, firstDirty });
        assert.equal(dirtyReads.length, 2, 'Eight dirty commits must coalesce behind the single pending read');
        assert.equal(dirtyReads[1].commitRevision, newest); assert.ok(dirtyReads[1].atMs - dirtyReads[0].atMs >= 100);
        await ownerPage.evaluate(() => window.uiHarness.releaseReadingProjection('dirty-newest'));
        await waitReadingQuery(ownerPage, { ...newOwner, commitRevision: newest });

        const continuousStart = await ownerPage.evaluate(() => {
          const port = window.uiHarness, start = port.readingProjectionQueries.length;
          port.controlledCommitTimer = setInterval(() => { const key = JSON.stringify([port.state.current.unitId, port.state.current.textRevision]), commitRevision = port.state.commitRevision + 1; port.readingProjectionFixtures[key].commitRevision = commitRevision; port.publish({ commitRevision }); }, 20);
          return start;
        });
        await ownerPage.waitForTimeout(650);
        assert.equal(await ownerPage.getByText('正在读取翻译状态…', { exact: true }).count(), 0, 'Continuous commits must retain an accepted as-of state');
        assert.equal(await ownerPage.getByText(/受控 owner · 翻译失败：owner_B/).isVisible(), true);
        const continuousEnd = await ownerPage.evaluate(start => { const port = window.uiHarness; clearInterval(port.controlledCommitTimer); return { commitRevision: port.state.commitRevision, reads: port.readingProjectionQueries.slice(start), maximum: [...port.readingProjectionMaxActive.values()] }; }, continuousStart);
        assert.ok(continuousEnd.reads.length >= 2 && continuousEnd.reads.length <= 8);
        for (let index = 1; index < continuousEnd.reads.length; index++) assert.ok(continuousEnd.reads[index].atMs - continuousEnd.reads[index - 1].atMs >= 100);
        assert.ok(continuousEnd.maximum.every(value => value === 1));
        await waitReadingQuery(ownerPage, { ...newOwner, commitRevision: continuousEnd.commitRevision });
        await ownerPage.waitForFunction(() => !document.body.textContent.includes('翻译状态更新中，以下保留上次读取结果。'));

        // A too-old start-floor or wrong-identity reply is visible and blocks
        // automatic retry until a new commit or explicit user action arrives.
        for (const mismatch of ['stale', 'profile', 'source']) {
          const reads = await ownerPage.evaluate(mismatch => {
            const port = window.uiHarness, key = JSON.stringify([port.state.current.unitId, port.state.current.textRevision]), value = structuredClone(port.readingProjectionFixtures[key]);
            const commitRevision = port.state.commitRevision + 1; port.readingProjectionFixtures[key].commitRevision = commitRevision; value.commitRevision = mismatch === 'stale' ? commitRevision - 1 : commitRevision;
            if (mismatch === 'profile') value.profileId = 'controlled-wrong-profile';
            if (mismatch === 'source') value.unit.sourceHash = 'f'.repeat(64);
            port.readingProjectionPlans.push({ unitId: value.unit.unitId, value }); port.publish({ commitRevision }); return port.readingProjectionQueries.length;
          }, mismatch);
          await ownerPage.getByRole('button', { name: '重新读取翻译状态', exact: true }).waitFor();
          await ownerPage.waitForTimeout(160);
          assert.equal(await ownerPage.evaluate(() => window.uiHarness.readingProjectionQueries.length), reads + 1, 'A rejected reply must not self-retry in a tight loop');
          await ownerPage.getByRole('button', { name: '重新读取翻译状态', exact: true }).click();
          await ownerPage.waitForFunction(() => !document.body.textContent.includes('翻译状态读取失败：'));
        }

        // RPC may lead the source snapshot. Buffer display is fenced by its
        // commit and exact attempt, independently of global historical status.
        const buffer = await ownerPage.evaluate(() => {
          const port = window.uiHarness, key = JSON.stringify([port.state.current.unitId, port.state.current.textRevision]), projection = structuredClone(port.readingProjectionFixtures[key]);
          projection.commitRevision = port.state.commitRevision + 2;
          const task = projection.translationState.engines[0].status.task; task.status = 'running'; task.errorCode = null; task.attemptId = 'controlled-attempt-new';
          port.readingProjectionFixtures[key] = projection;
          port.publish({ commitRevision: projection.commitRevision - 1, tasks: [{ ...task, partialText: '受控新 attempt 片段' }] });
          return { unitId: projection.unit.unitId, textRevision: projection.unit.textRevision, commitRevision: projection.commitRevision - 1, rpcCommit: projection.commitRevision };
        });
        await waitReadingQuery(ownerPage, buffer);
        assert.equal(await ownerPage.getByText('受控新 attempt 片段', { exact: true }).count(), 0, 'A source snapshot older than RPC cannot supply a partial');
        const tokenReads = await ownerPage.evaluate(commitRevision => { const port = window.uiHarness, count = port.readingProjectionQueries.length; port.publish({ commitRevision }); return count; }, buffer.rpcCommit);
        await ownerPage.getByText('受控新 attempt 片段', { exact: true }).waitFor();
        await ownerPage.evaluate(() => { const port = window.uiHarness; port.publish({ tasks: port.state.tasks.map(task => ({ ...task, attemptId: 'controlled-attempt-old', partialText: '受控旧 attempt 片段' })) }); });
        await ownerPage.waitForFunction(() => !document.body.textContent.includes('受控新 attempt 片段'));
        assert.equal(await ownerPage.getByText('受控旧 attempt 片段', { exact: true }).count(), 0);
        await ownerPage.evaluate(() => { const port = window.uiHarness; for (let index = 0; index < 12; index++) port.publish({ tasks: port.state.tasks.map(task => ({ ...task, attemptId: 'controlled-attempt-new', partialText: `受控 token ${index}` })) }); });
        await ownerPage.getByText('受控 token 11', { exact: true }).waitFor();
        await ownerPage.waitForTimeout(160);
        assert.equal(await ownerPage.evaluate(() => window.uiHarness.readingProjectionQueries.length), tokenReads, 'Token-only snapshot revisions must not refetch business state');
      } finally { await ownerPage.close(); }
    }
    const visibilityPage = await appPage();
    try {
      assert.equal(await visibilityPage.evaluate(() => window.uiHarness.readingProjectionQueries.length), 0, 'Hidden MainWindow features must not query reading state');
      await visibilityPage.getByRole('button', { name: '当前会话', exact: true }).click();await openSessionDataset(visibilityPage,'正文历史');
      await visibilityPage.waitForFunction(() => window.uiHarness.readingProjectionQueries.length === 1);
      await visibilityPage.getByRole('button', { name: '游戏库', exact: true }).click();
      const inactiveReads = await visibilityPage.evaluate(() => { const port = window.uiHarness, count = port.readingProjectionQueries.length; port.publish({ commitRevision: port.state.commitRevision + 1 }); return count; });
      await visibilityPage.waitForTimeout(180);
      assert.equal(await visibilityPage.evaluate(() => window.uiHarness.readingProjectionQueries.length), inactiveReads);
      await visibilityPage.getByRole('button', { name: '手动翻译', exact: true }).click();
      await visibilityPage.waitForFunction(count => window.uiHarness.readingProjectionQueries.length > count, inactiveReads);
    } finally { await visibilityPage.close(); }

    // R68: the real production HUD must compose its empty translation surface
    // with the same query owner. This extends RBC38; no old assertion is removed.
    const placeholderObservations = [];
    for (const width of [560, 500]) {
      const hud = await appPage('hud', width, 500, port => {
        const unit = { ...port.state.current, translations: [] };
        port.state = { ...port.state, current: unit, history: { ...port.state.history, units: [unit] }, tasks: [] };
        port.readingProjectionPlans.push({ holdKey: 'r68-initial-query' });
        port.dictionaryEntries = [{ entryId: 'r68-fixed-entry', lemma: '放課後', reading: 'ほうかご', senses: [{ language: 'zh', text: '受控R68固定词义', usageNotes: null }], sourceLabel: 'R68组件词典输入', sourceVersion: 'controlled-r68', licenseRef: 'component-fixture', attribution: 'controlled input; not real resource acceptance' }];
      });
      try {
        const surface = hud.locator('[data-ui-scroll="reading"]');
        const noGeneric = async reason => assert.equal(await surface.getByText('尚无译文。翻译用途未配置、未启用或仍在处理时，原文保持可读。', { exact: true }).count(), 0, reason);
        const settle = async () => { await hud.waitForFunction(() => [...window.uiHarness.readingProjectionActive.values()].every(count => count === 0)); await hud.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); };
        const extent = () => surface.evaluate(node => ({ height: node.firstElementChild.getBoundingClientRect().height, fits: [...window.uiHarness.hudFitRequests] }));
        await hud.waitForFunction(() => window.uiHarness.readingProjectionPending.has('r68-initial-query'));
        await noGeneric('An initial held state query cannot claim that a real translation is absent');
        const liveStatus = hud.getByRole('status', { name: '翻译状态', exact: true });
        assert.match(await liveStatus.ariaSnapshot(), /正在读取翻译状态/);
        assert.equal(await liveStatus.evaluate(node => getComputedStyle(node).position), 'absolute');
        await liveStatus.evaluate(node => { window.r68PersistentStatus = node; });
        await hud.evaluate(() => window.uiHarness.releaseReadingProjection('r68-initial-query')); await settle();
        await surface.getByText('当前原句暂无翻译任务。', { exact: true }).waitFor();
        // Visible source and its actual word-card anchor stay mounted when
        // display modes change; source-only never activates the query owner.
        const sourceNode = surface.getByTestId('source-text'), anchor = sourceNode.getByRole('button').first();
        await sourceNode.evaluate(node => { window.r68SourceNode = node; });
        await anchor.evaluate(node => { window.r68SourceAnchor = node; }); await anchor.click();
        await hud.getByRole('dialog', { name: '本地词典', exact: true }).getByText('受控R68固定词义', { exact: true }).waitFor();
        const frozenWordCard = await hud.evaluate(() => ({ card: structuredClone(window.uiHarness.wordCard), rect: structuredClone(window.uiHarness.wordCardRect), requests: structuredClone(window.uiHarness.wordCardRequests) }));
        for (const mode of ['source', 'both', 'translation', 'source', 'both']) {
          await hud.evaluate(mode => { const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, readingDisplayMode: mode } }); }, mode); await settle();
          assert.equal(await sourceNode.evaluate(node => node === window.r68SourceNode), true, 'Display-mode changes preserve the still-visible SourceText node');
          assert.equal(await anchor.evaluate(node => node === window.r68SourceAnchor && node.isConnected), true, 'A visible word-card anchor must not remount across mode changes');
          assert.deepEqual(await hud.evaluate(() => ({ card: structuredClone(window.uiHarness.wordCard), rect: structuredClone(window.uiHarness.wordCardRect), requests: structuredClone(window.uiHarness.wordCardRequests) })), frozenWordCard, 'The frozen card object and its open request remain unchanged');
          if (mode === 'source') {
            const reads = await hud.evaluate(() => { const port = window.uiHarness, reads = port.readingProjectionQueries.length; port.publish({ commitRevision: port.state.commitRevision + 1 }); return reads; }); await hud.waitForTimeout(180);
            assert.equal(await hud.evaluate(() => window.uiHarness.readingProjectionQueries.length), reads, 'Source-only mode must not query translation state');
            assert.equal(await hud.getByRole('status', { name: '翻译状态', exact: true }).count(), 0);
          }
        }
        await hud.keyboard.press('Escape'); await hud.waitForFunction(() => window.uiHarness.wordCard === null);
        await liveStatus.evaluate(node => { window.r68PersistentStatus = node; });
        const assertTranslationOnly = async () => {
          assert.equal(await surface.getByTestId('source-text').count(), 0, 'A completed authoritative result hides the source in translation-only mode');
          assert.equal(await surface.getByText('当前尚无完整译文，继续保留原文。', { exact: true }).count(), 0, 'A completed authoritative result cannot coexist with the no-full-translation fallback');
        };
        const publishState = async (phase, options = {}) => {
          await hud.evaluate(({ phase, options }) => {
            const port = window.uiHarness, base = port.state.current, commitRevision = port.state.commitRevision + 1;
            const task = { taskId: 'r68-task', unitId: base.unitId, textRevision: base.textRevision, purpose: 'translation', status: phase, errorCode: phase === 'failed' ? 'controlled_r68_failure' : null, partialText: null, providerLabel: '受控R68引擎', attemptId: 'r68-attempt' };
            const translations = options.final ? [{ translationId: 'r68-final', taskId: task.taskId, text: options.final, providerLabel: task.providerLabel, targetLanguage: 'zh-CN', correction: null }] : [];
            const unit = { ...base, translations };
            const status = phase === 'unrequested' ? { kind: 'unrequested' } : phase === 'unavailable' ? { kind: 'unavailable', error: { code: 'controlled_unavailable', stage: 'storage', message: '受控R68状态无法核验', recoverable: true } } : phase === 'unordered' ? { kind: 'unordered', summary: { headCount: '2', queuedCount: '0', runningCount: options.partial ? '1' : '0', completedCount: '0', failedCount: '1', cancelledCount: '0', invalidatedCount: '0' }, activeTask: options.partial ? { ...task, status: 'running' } : null } : { kind: 'current', task };
            const engines = phase === 'empty' ? [] : [{ scopeKey: 'global', bindingId: 'r68-binding', engineId: 'r68-engine', providerLabel: task.providerLabel, status }];
            port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = { profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit, translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, engines } };
            port.publish({ commitRevision, current: unit, history: { ...port.state.history, units: [unit] }, tasks: options.partial ? [{ ...task, status: 'running', partialText: options.partial }] : [], session: { ...port.state.session, translationEnabled: !options.disabled } });
          }, { phase, options }); await settle();
        };
        for (const [phase, text] of [['queued', '等待翻译'], ['running', '翻译中']]) {
          await publishState(phase); await surface.getByText(`受控R68引擎 · ${text}`, { exact: true }).waitFor(); await noGeneric(`Real ${phase} progress cannot coexist with the generic missing-result notice`);
          assert.equal(await surface.getByRole('button', { name: '取消', exact: true }).isVisible(), true);
        }
        await publishState('running', { partial: '受控R68流式片段' }); await surface.getByText('受控R68流式片段', { exact: true }).waitFor(); await noGeneric('A real matching stream fragment replaces the generic empty fallback');
        assert.equal(await surface.getByText('受控R68流式片段', { exact: true }).evaluate(node => getComputedStyle(node).fontSize), await hud.evaluate(() => `${window.uiHarness.state.preferences.translationFontPt}px`), 'Partial typography is unchanged');
        await screenshot(hud, `r68-hud-running-partial-${width}`);
        await publishState('completed', { final: '受控R68完成译文' }); await surface.getByText('受控R68完成译文', { exact: true }).waitFor(); await noGeneric('Completed translation renders actual text');
        assert.equal(await surface.getByText('受控R68流式片段', { exact: true }).count(), 0);
        for (const [phase, label] of [['failed', '翻译失败：controlled_r68_failure'], ['cancelled', '翻译已取消'], ['invalidated', '旧翻译已失效']]) {
          await publishState(phase); await surface.getByText(`受控R68引擎 · ${label}`, { exact: true }).waitFor(); await noGeneric(`Real ${phase} status must remain visible without a contradictory fallback`);
        }
        await publishState('unavailable'); await surface.getByText('受控R68引擎 · 翻译状态无法核验：受控R68状态无法核验', { exact: true }).waitFor(); await noGeneric('Unverifiable status is explicit');
        await publishState('unordered', { partial: '受控R68归属未定片段' }); await surface.getByText(/受控R68引擎 · 请求归属未定/).waitFor(); await surface.getByText('受控R68归属未定片段', { exact: true }).waitFor(); await noGeneric('Unordered active progress stays truthful');
        await publishState('unrequested'); await surface.getByText('受控R68引擎 · 尚未发起翻译。', { exact: true }).waitFor(); await noGeneric('Unrequested status cannot guess a missing configuration');
        const settings = surface.getByRole('button', { name: '检查翻译设置…', exact: true }); await settings.click(); assert.equal(await hud.evaluate(() => window.uiHarness.windows.at(-1)), 'settings');
        // An unchanged as-of empty state and the persistent query slot must not
        // alter natural height or fit requests during an unrelated held refresh.
        const stable = await extent();
        await hud.evaluate(() => { const port = window.uiHarness; port.readingProjectionPlans.push({ holdKey: 'r68-empty-refresh' }); const commitRevision = port.state.commitRevision + 1; port.readingProjectionFixtures[JSON.stringify([port.state.current.unitId, port.state.current.textRevision])].commitRevision = commitRevision; port.publish({ commitRevision }); });
        await hud.waitForFunction(() => window.uiHarness.readingProjectionPending.has('r68-empty-refresh')); await hud.waitForTimeout(100);
        const held = await extent(); assert.equal(held.height, stable.height); assert.deepEqual(held.fits, stable.fits); await noGeneric('Background refresh keeps the same truthful empty state');
        assert.equal(await liveStatus.evaluate(node => node === window.r68PersistentStatus), true);
        await hud.evaluate(() => window.uiHarness.releaseReadingProjection('r68-empty-refresh')); await settle(); const settled = await extent(); assert.equal(settled.height, stable.height); assert.deepEqual(settled.fits, stable.fits);
        placeholderObservations.push({ width, scenario: 'unrequested-held-refresh', stable, held, settled });
        await publishState('empty', { disabled: true }); await surface.getByText('自动翻译已关闭，原文保持可读。', { exact: true }).waitFor(); await noGeneric('Only matching authoritative session state may report automatic translation disabled');
        await publishState('empty'); await surface.getByText('当前原句暂无翻译任务。', { exact: true }).waitFor(); await noGeneric('Zero projected engines do not establish why configuration is absent');
        await hud.evaluate(() => { const port = window.uiHarness; port.r68MatchingSession = structuredClone(port.state.session); port.publish({ session: { ...port.state.session, sessionId: 'r68-unrelated-disabled-session', translationEnabled: false } }); });
        await settle(); await surface.getByText('当前原句暂无翻译任务。', { exact: true }).waitFor(); assert.equal(await surface.getByText('自动翻译已关闭，原文保持可读。', { exact: true }).count(), 0, 'A disabled unrelated session cannot explain this fixed reading object');
        await hud.evaluate(() => window.uiHarness.publish({ session: window.uiHarness.r68MatchingSession })); await settle();

        await publishState('completed'); await surface.getByText('受控R68引擎 · 翻译请求已完成，当前没有可显示的译文。', { exact: true }).waitFor(); await noGeneric('A completed task without visible result is represented explicitly');
        await hud.evaluate(() => { const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, readingDisplayMode: 'translation' } }); }); await settle();
        // Equal persistent commit still carries the caller's current runtime
        // snapshot. A tied as-of query must not replace that reading content.
        await publishState('running');
        await hud.evaluate(() => { const port = window.uiHarness, unit = { ...port.state.current, translations: [{ translationId: 'r68-equal-commit', taskId: 'r68-task', text: '受控R68同提交快照内容', providerLabel: '受控R68引擎', targetLanguage: 'zh-CN', correction: null }] }; port.publish({ current: unit }); });
        await surface.getByText('受控R68同提交快照内容', { exact: true }).waitFor(); await noGeneric('Equal commit must preserve the caller snapshot reading content'); await assertTranslationOnly();
        // Snapshot completes before an older empty query response: rendering
        // must never select by array length or clear a newer committed result.
        await publishState('running');
        await hud.evaluate(() => { const port = window.uiHarness; const commitRevision = port.state.commitRevision + 1, unit = port.state.current; port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])].commitRevision = commitRevision; port.readingProjectionPlans.push({ holdKey: 'r68-snapshot-first-old' }); port.publish({ commitRevision }); });
        await hud.waitForFunction(() => window.uiHarness.readingProjectionPending.has('r68-snapshot-first-old'));
        await hud.evaluate(() => { const port = window.uiHarness, commitRevision = port.state.commitRevision + 1, translation = { translationId: 'r68-snapshot-new', taskId: 'r68-task', text: '受控R68快照先完成', providerLabel: '受控R68引擎', targetLanguage: 'zh-CN', correction: null }; const unit = { ...port.state.current, translations: [translation] }; const projection = port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])]; projection.commitRevision = commitRevision; projection.unit = unit; projection.translationState.engines[0].status.task.status = 'completed'; port.readingProjectionPlans.push({ holdKey: 'r68-snapshot-first-next' }); port.publish({ commitRevision, current: unit, history: { ...port.state.history, units: [unit] } }); });
        await surface.getByText('受控R68快照先完成', { exact: true }).waitFor(); await hud.evaluate(() => window.uiHarness.releaseReadingProjection('r68-snapshot-first-old'));
        await hud.waitForFunction(() => window.uiHarness.readingProjectionPending.has('r68-snapshot-first-next')); await surface.getByText('受控R68快照先完成', { exact: true }).waitFor(); await noGeneric('Late older as-of query cannot clear the newer snapshot result'); await assertTranslationOnly();
        await hud.evaluate(() => window.uiHarness.releaseReadingProjection('r68-snapshot-first-next')); await settle();
        // The reverse race: query is newer than the snapshot and is authoritative
        // for this exact fixed object; completed text cannot be hidden as empty.
        await publishState('running');
        await hud.evaluate(() => { const port = window.uiHarness, commitRevision = port.state.commitRevision + 1, unit = port.state.current, projected = { ...unit, translations: [{ translationId: 'r68-query-new', taskId: 'r68-task', text: '受控R68查询先完成', providerLabel: '受控R68引擎', targetLanguage: 'zh-CN', correction: null }] }; const projection = port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])]; projection.commitRevision = commitRevision + 1; projection.unit = projected; projection.translationState.engines[0].status.task.status = 'completed'; port.publish({ commitRevision }); }); await settle();
        await surface.getByText('受控R68查询先完成', { exact: true }).waitFor(); assert.equal(await hud.evaluate(() => window.uiHarness.state.current.translations.length), 0); await noGeneric('Newer same-identity projection provides its real completed text'); await assertTranslationOnly();
        // Query errors preserve actual visible text and keep the existing retry.
        await hud.evaluate(() => { const port = window.uiHarness, projected = port.readingProjectionFixtures[JSON.stringify([port.state.current.unitId, port.state.current.textRevision])], commitRevision = port.state.commitRevision + 2; projected.commitRevision = commitRevision; port.readingProjectionPlans.push({ error: '受控R68读取失败' }); port.publish({ commitRevision, current: structuredClone(projected.unit) }); });
        const alert = surface.getByRole('alert').filter({ hasText: '受控R68读取失败' }); await alert.waitFor(); assert.equal(await alert.isVisible(), true); await noGeneric('Query failure remains explicit'); assert.equal(await surface.getByText('受控R68查询先完成', { exact: true }).isVisible(), true, 'Query failure keeps the committed completed text');
        const failedReads = await hud.evaluate(() => window.uiHarness.readingProjectionQueries.length);
        await surface.getByRole('button', { name: '重新读取翻译状态', exact: true }).click();
        await hud.waitForFunction(count => window.uiHarness.readingProjectionQueries.length > count, failedReads); await settle();
        await hud.waitForFunction(() => !document.body.textContent.includes('翻译状态读取失败：'));
        assert.equal(await surface.getByRole('alert').filter({ hasText: '翻译状态读取失败：' }).count(), 0, 'Retry must clear every query failure, including stale-floor errors');
        assert.equal(await surface.getByText('受控R68查询先完成', { exact: true }).isVisible(), true); await assertTranslationOnly();
        assert.equal(await hud.evaluate(() => [...window.uiHarness.readingProjectionActive.values()].every(count => count === 0)), true, 'The successful retry query actually settled');
        assert.ok(await hud.evaluate(() => [...window.uiHarness.readingProjectionMaxActive.values()].every(count => count === 1)), 'Composition must retain a single query owner per exact identity');
        await screenshot(hud, `r68-hud-query-newer-${width}`);
      } finally { await hud.close(); }
    }
    await fs.writeFile(path.join(out, 'r68-translation-placeholder-observations.json'), JSON.stringify(placeholderObservations, null, 2) + '\n');
  });
  await check('RBC-39-cover-decodes-real-bytes-and-reads-once-per-reference',async()=>{
    const p=await appPage(),identity=await prepareCover(p),image=p.getByRole('img',{name:'Controlled local cover 封面',exact:true});
    await image.waitFor();assert.deepEqual(await image.evaluate(async image=>{await image.decode();return [image.complete,image.naturalWidth,image.naturalHeight];}),[true,240,160]);assert.match(await image.getAttribute('src'),/^blob:/);assert.equal(await image.getAttribute('aria-description'),'本机 Steam 封面');await p.locator('[data-cover-state="ready"]').waitFor();
    const source=await image.getAttribute('src');assert.deepEqual(await p.evaluate(()=>window.uiHarness.coverRequests),[{gameId:identity.gameId,expectedAssetRef:identity.assetRef}]);
    await p.evaluate(()=>{const port=window.uiHarness;for(let i=0;i<15;i++)port.publish({current:{...port.state.current,sourceText:`受控字幕变化${i}`},games:port.state.games.map(game=>({...game,rowRevision:game.rowRevision+1}))});});
    await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await p.evaluate(()=>window.uiHarness.coverRequests.length),1);assert.equal(await image.getAttribute('src'),source);
    await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await p.getByRole('button',{name:'游戏库',exact:true}).click();await image.waitFor();assert.equal(await p.evaluate(()=>window.uiHarness.coverRequests.length),1,'Navigating back must reuse the immutable-reference bytes');
    await p.evaluate(()=>{const port=window.uiHarness,game=port.state.games[0],assetRef='managed-cover:'+('b'.repeat(64));port.coverFixtures[game.gameId]={...port.coverFixtures[game.gameId],assetRef,contentHash:'b'.repeat(64)};port.publish({games:[{...game,coverAssetRef:assetRef}]});});
    await p.waitForFunction(()=>window.uiHarness.coverRequests.length===2);await p.locator('[data-cover-state="ready"]').waitFor();assert.equal(await image.evaluate(async image=>{await image.decode();return image.naturalWidth;}),240);assert.equal(await p.evaluate(()=>window.uiHarness.coverRequests.at(-1).expectedAssetRef),'managed-cover:'+('b'.repeat(64)));await screenshot(p,'library-cover-real-jpeg-decodes');await p.close();
  });
  await check('RBC-40-corrupt-missing-and-wrong-owner-covers-show-fallback',async()=>{
    for(const [mode,message] of [['corrupt','封面图像无法显示'],['missing','尚无封面 · 连接后自动获取游戏画面'],['wrong-owner','封面读取失败']]){
      const p=await appPage();await prepareCover(p,mode);await p.getByText(message,{exact:true}).waitFor();assert.equal(await p.locator('[data-cover-state="placeholder"]').count(),1);assert.equal(await p.getByRole('img',{name:'Controlled local cover 封面',exact:true}).count(),0);assert.equal(await p.evaluate(()=>window.uiHarness.coverRequests.length),1);assert.equal(await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).isEnabled(),true);if(mode==='corrupt')await screenshot(p,'library-corrupt-cover-fallback');await p.close();
    }
  });
  await check('RBC-41-running-only-same-panel-one-confirm-and-explicit-stream',async()=>{
    const p=await appPage('main',1040,900,port=>{port.state.session=null;port.state.current=null;port.state.history.units=[];port.bindingFixture={...port.bindingFixture,state:'unbound',sessionId:null,targetIdentitySha256:null};});await prepareCover(p,'missing');
    for(const name of ['运行并连接…','关联并接入…','连接已运行的游戏…'])assert.equal(await p.getByRole('button',{name,exact:true}).count(),0);
    assert.equal(await p.evaluate(()=>window.uiHarness.launches.length),0);
    for(const file of auditedFiles.filter(file=>file.startsWith(path.join(desktop,'src'))&&/\.(tsx?|mjs)$/.test(file)))assert.doesNotMatch(await fs.readFile(file,'utf8'),/\blaunchGame\s*\(|['"]launch_game['"]/,`Client launch entry remains: ${path.relative(root,file)}`);
    await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).click();const dialog=p.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await dialog.getByRole('radio').first().check();
    assert.equal(await dialog.getByRole('region',{name:'关联与实际取文目标',exact:true}).count(),1);assert.equal(await dialog.getByText('目标完整路径',{exact:true}).count(),1);assert.equal(await dialog.getByText('C:\\UI-test\\Game.exe',{exact:true}).count(),1);assert.equal(await dialog.getByRole('button',{name:'核对目标',exact:true}).count(),0);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);
    await screenshot(p,'binding-single-panel-before-one-confirm');await dialog.getByRole('button',{name:'关联并接入',exact:true}).click();await p.getByRole('dialog',{name:'正在接入游戏',exact:true}).waitFor();assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),1);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='select_source').length),0);await p.getByRole('list',{name:'自动接入阶段',exact:true}).waitFor();assert.equal(await p.locator('[aria-label="自动接入阶段"] [aria-current="step"]').filter({hasText:'已连接'}).count(),1);assert.equal(await p.evaluate(()=>window.uiHarness.launches.length),0);
    await p.evaluate(()=>{const port=window.uiHarness;port.bindingJourneyFixture.stage='candidates_ready';port.bindingJourneyFixture.sequence++;port.publish({session:{...port.state.session,sources:[{sourceId:'new-explicit-source',hookName:'真实候选受控传输',hookCode:'controlled',sample:'まだ選択していない。',selected:false,encoding:'UTF-16'}]}});});
    assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='select_source').length),0);await p.getByRole('checkbox',{name:'选择文本流 真实候选受控传输',exact:true}).check();assert.equal(await p.evaluate(()=>window.uiHarness.bindingJourneyFixture.beginRequests.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='select_source').length),0);await p.getByRole('button',{name:'开始阅读',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.bindingJourneyFixture.beginRequests.length===1);assert.deepEqual(await p.evaluate(()=>window.uiHarness.bindingJourneyFixture.beginRequests[0].sourceIds),['new-explicit-source']);assert.equal(await p.evaluate(()=>window.uiHarness.state.session.sources[0].selected),true);await p.close();
  });
  await check('RBC-42-candidate-preview-starts-automatically-and-cancel-releases-only-candidate',async()=>{
    const p=await appPage();await prepareCover(p,'missing');const before=await p.evaluate(()=>structuredClone(window.uiHarness.state.session));await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).click();const dialog=p.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await dialog.getByRole('radio').first().check();await p.waitForFunction(()=>window.uiHarness.previewRequests.length===1);
    const request=await p.evaluate(()=>window.uiHarness.previewRequests[0]);assert.equal(request.consumer,'candidate');assert.equal(request.candidateId,'ui-target');assert.equal(await dialog.getByRole('button',{name:'开启窗口预览',exact:true}).count(),0);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);const lease=await p.evaluate(()=>[...window.uiHarness.previewProjections.values()].find(value=>value.consumerId===window.uiHarness.previewRequests[0].consumerId).leaseId);await dialog.getByRole('button',{name:'取消',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.previewControls.some(control=>control.action==='stop'));
    assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.session),before);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.deepEqual(await p.evaluate(()=>window.uiHarness.previewControls.filter(control=>control.action==='stop').map(control=>control.leaseId)),[lease]);assert.equal(await p.evaluate(()=>window.uiHarness.previewProjections.get('ui-session-lease-1').state),'starting');await p.close();
  });
  await check('RBC-43-session-preview-persists-across-tabs-and-navigation-without-stop',async()=>{
    const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===1);const lease=await emitSessionFrame(p),preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await preview.locator('img').waitFor();const image=await preview.locator('img').getAttribute('src');
    for(const name of ['全部提取','Hook 日志','正文历史']){await openSessionDataset(p,name,{exclusive:false});assert.equal(await preview.locator('img').getAttribute('src'),image);assert.equal(await p.evaluate(()=>window.uiHarness.previewAttaches.length),1);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.filter(item=>item.action==='stop').length),0);}
    await p.getByRole('button',{name:'游戏库',exact:true}).click();assert.equal(await p.evaluate(lease=>window.uiHarness.previewProjections.get(lease).state,lease),'live');assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.filter(item=>item.action==='stop').length),0);await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===2);await preview.locator('img').waitFor();assert.equal(await preview.locator('img').evaluate(async image=>{await image.decode();return image.naturalWidth;}),160);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);assert.equal(await p.evaluate(lease=>window.uiHarness.previewProjections.get(lease).state,lease),'live');assert.deepEqual(await p.evaluate(()=>window.uiHarness.previewAttaches),[lease,lease]);await screenshot(p,'session-preview-constant-across-tabs');
    await preview.getByRole('button',{name:'暂停',exact:true}).click();await p.locator('[data-preview-state="paused"]').waitFor();const retained=await p.evaluate(()=>structuredClone(window.uiHarness.savedPreviewUpdate.frame));assert.equal(await p.evaluate(()=>window.uiHarness.previewAttaches.length),3,'Explicit pause advances generation and reattaches only that subscription');
    const pauseSubscriptions=await p.evaluate(()=>{const port=window.uiHarness;port.previewSubscriptionHistory[1].unsubscribe();port.previewSubscriptionHistory[1].unsubscribe();return {history:port.previewSubscriptionHistory.map(({id,projection})=>({id,leaseId:projection.leaseId,generation:projection.generation})),active:[...port.previewListeners.values()].map(({projection})=>({leaseId:projection.leaseId,generation:projection.generation})),detaches:port.previewDetaches.length};});assert.equal(new Set(pauseSubscriptions.history.map(v=>v.id)).size,3);assert.equal(pauseSubscriptions.history[0].generation,pauseSubscriptions.history[1].generation);assert.notEqual(pauseSubscriptions.history[1].generation,pauseSubscriptions.history[2].generation);assert.deepEqual(pauseSubscriptions.active,[{leaseId:lease,generation:pauseSubscriptions.history[2].generation}]);assert.equal(pauseSubscriptions.detaches,2,'An already retired closure cannot detach the new paused generation');
    await p.getByRole('button',{name:'游戏库',exact:true}).click();assert.equal(await p.evaluate(lease=>window.uiHarness.previewProjections.get(lease).state,lease),'paused');await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===4);await preview.locator('img').waitFor();assert.equal(await preview.getAttribute('data-preview-state'),'paused');assert.equal(await preview.locator('img').evaluate(async image=>{await image.decode();return image.naturalWidth;}),160);
    const restored=await p.evaluate(lease=>{const port=window.uiHarness,projection=port.previewProjections.get(lease);return port.previewFrameFixtures.get(`${lease}:${projection.generation}`);},lease);assert.deepEqual({...restored,generation:retained.generation},retained);assert.notEqual(restored.generation,retained.generation);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.filter(item=>item.action==='stop').length),0);const afterNavigation=await p.evaluate(()=>({active:window.uiHarness.previewListeners.size,history:window.uiHarness.previewSubscriptionHistory.map(({id,projection})=>({id,generation:projection.generation})),detaches:window.uiHarness.previewDetaches.length}));assert.equal(afterNavigation.active,1);assert.equal(afterNavigation.detaches,3);assert.equal(new Set(afterNavigation.history.map(v=>v.id)).size,4);assert.equal(afterNavigation.history[3].generation,pauseSubscriptions.history[2].generation);await screenshot(p,'paused-preview-retained-after-navigation');await p.close();
    // Real production adapter, exact same lease across a control generation.
    // The old in-flight read and its closure must not cancel or duplicate the new pull.
    const fixture=await adapterFixture(),oldUpdates=[],newUpdates=[];let releaseOld;
    try{fixture.replies.push(()=>new Promise(resolve=>{const old=fixture.update();releaseOld=()=>resolve(old);}));const offOld=fixture.attach(update=>oldUpdates.push(update));await waitUntil(()=>!!releaseOld);fixture.projection={...fixture.projection,generation:'2',state:'paused'};fixture.attach(update=>newUpdates.push(update));assert.equal(fixture.calls.length,1);assert.equal(fixture.maxInFlight,1);offOld();offOld();releaseOld();await waitUntil(()=>newUpdates.length>=2);assert.equal(oldUpdates.filter(update=>update.frame).length,0);assert.ok(newUpdates.every(update=>update.projection.generation==='2'));assert.equal(fixture.maxInFlight,1);assert.equal(fixture.calls.filter(call=>call.args.generation==='1').length,1);assert.equal(fixture.calls.filter(call=>call.args.generation==='2')[0].args.afterSequence,null);assert.equal(fixture.calls.some(call=>call.name!=='application_preview_latest'),false);fixture.close();await waitUntil(()=>fixture.inFlight===0);const readCount=fixture.calls.length;await new Promise(resolve=>setTimeout(resolve,650));assert.equal(fixture.calls.length,readCount,'Every old and new subscription is released; no orphan read loop remains');}finally{fixture.close();}
  });
  await check('RBC-44-diagnostic-default-and-full-log-query-search-paging-export',async()=>{
    const p=await appPage();await p.evaluate(()=>{const port=window.uiHarness,id=port.state.session.sessionId;const fact=(sequence,code,stage,level,message)=>({kind:'hook_event',sequence:String(sequence),eventId:`diagnostic-${sequence}`,operationId:'controlled-operation',sessionId:id,sourceId:null,wireSequence:'18446744073709551615',observedAtMs:1792368000000+sequence,level,stage,code,message,lostCount:null,integrity:'intact'});port.sessionRecordFixtures[`${id}:hook_events`]=[...Array.from({length:80},(_,i)=>fact(i+1,'core_log','injection','info',`独立诊断·${String(i+1).padStart(4,'0')}`)),...Array.from({length:130},(_,i)=>fact(i+81,['text_unselected','text_paused','text_empty','text_inactive'][i%4],'capture','info',`保存回调${i}`)),fact(211,'text_unselected','capture','warning','上游警告必须保留')];});
    await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await openSessionDataset(p,'Hook 日志');await p.waitForFunction(()=>window.uiHarness.sessionRecordQueries.some(query=>query.dataset==='hook_events'));assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.find(query=>query.dataset==='hook_events').filter.stage),'diagnostics');await waitSessionSummaryCount(p,'logs','81');assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);assert.equal(await p.locator('[data-record-sequence="211"]').count(),1);
    await p.getByRole('button',{name:'加载较早记录',exact:true}).click();await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===81);assert.equal(await p.locator('details[data-flat-session-section="logs"] [data-record-sequence="1"]').count(),1);
    const scope=p.getByRole('combobox',{name:'事件范围',exact:true});await scope.click();await p.getByRole('option',{name:'全部事件',exact:true}).click();await waitSessionSummaryCount(p,'logs','211');assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.at(-1).filter.stage),null);assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);
    await p.getByRole('textbox',{name:'搜索 Hook 日志',exact:true}).fill('独立诊断·0001');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===1);assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').getAttribute('data-record-sequence'),'1');await p.getByRole('textbox',{name:'搜索 Hook 日志',exact:true}).fill('');await scope.click();await p.getByRole('option',{name:'接入诊断',exact:true}).click();await waitSessionSummaryCount(p,'logs','81');
    for(const [filtered,count,stage] of [[false,'211',null],[true,'81','diagnostics']]){await p.getByRole('button',{name:'导出…',exact:true}).click();const dialog=p.getByRole('dialog',{name:'导出当前会话',exact:true});if(filtered){await dialog.getByRole('combobox',{name:'导出记录范围',exact:true}).click();await p.getByRole('option',{name:'当前过滤结果',exact:true}).click();}await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'核对导出范围',exact:true}).click();await dialog.getByRole('button',{name:'选择保存位置并导出',exact:true}).waitFor();const plan=await p.evaluate(()=>[...window.uiHarness.sessionExportPlans.values()].at(-1));assert.equal(plan.snapshot.matchingCount,count);assert.equal(plan.filter.stage,stage);await dialog.getByRole('button',{name:'取消',exact:true}).click();}
    assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordFixtures[`${window.uiHarness.state.session.sessionId}:hook_events`].length),211);await screenshot(p,'hook-diagnostics-default-complete-query');await p.close();
  });
  await check('RBC-45-preview-permission-recovery-is-automatic-and-hook-independent',async()=>{
    const p=await appPage('main',1040,760,port=>{port.previewCapabilityFixture={supported:true,permission:'denied',errorCode:'screen_capture_permission',detail:'Controlled initial denial'};const projection=[...port.previewProjections.values()].at(-1);projection.state='permission_revoked';projection.permission='denied';});await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await preview.locator('[data-preview-state]').count();await p.locator('[data-preview-state="permission_revoked"]').waitFor();const before=await p.evaluate(()=>structuredClone(window.uiHarness.state.session));assert.equal(await preview.getByRole('button',{name:'开启窗口预览',exact:true}).count(),0);
    await p.evaluate(()=>{const port=window.uiHarness;port.previewCapabilityFixture={supported:true,permission:'granted',errorCode:null,detail:'Controlled permission recovery'};const projection=[...port.previewProjections.values()].find(value=>value.consumerId===port.state.session.sessionId&&value.state!=='stopped');projection.generation=String(++port.previewGeneration);projection.state='starting';projection.permission='granted';});await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===2);await emitSessionFrame(p);await preview.locator('img').waitFor();assert.equal(await preview.locator('img').evaluate(async image=>{await image.decode();return image.naturalWidth;}),160);assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.session),before);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);await screenshot(p,'preview-auto-recovers-after-grant');await p.close();
  });
  await check('RBC-46-real-tauri-transport-failure-and-candidate-recovery-do-not-leak',async()=>{
    // Run the actual production adapter against a controlled invoke transport;
    // this exercises its catch/stop path rather than reproducing that logic.
    const calls=[],received=[];let failLatest=true,latestFrame=null;
    const projection={leaseId:'native-adapter-candidate',generation:'1',consumerId:'candidate-owner',state:'starting',permission:'granted',targetIdentityHash:'f'.repeat(64),attemptedAtMs:null,capturedAtMs:null,contentChangedAtMs:null,permissionCheckedAtMs:null,errorCode:null,detail:'Controlled invoke transport'};
    const invoke=async(name,args)=>{calls.push({name,args});if(name==='application_preview_start')return projection;if(name==='application_preview_latest'){if(failLatest)throw Error('controlled native transport failure');return {projection:{...projection,leaseId:args.leaseId},frame:latestFrame};}if(name==='application_preview_control')return {...projection,state:'stopped'};throw Error(`Unexpected native call ${name}`);};
    const stubs={'@tauri-apps/api/event':{listen:async()=>()=>{}},'@tauri-apps/api/core':{Channel:class{},invoke},'@tauri-apps/api/window':{getCurrentWindow:()=>({})},'@tauri-apps/api/webview':{getCurrentWebview:()=>({})}};
    const adapter=loadActualTauriPort(stubs);
    const started = await adapter.startWindowPreview({candidateId:'controlled-candidate',consumer:'candidate',consumerId:projection.consumerId,expectedGeneration:null});assert.equal(calls.filter(call=>call.name==='application_preview_latest').length,0,'Control must not install a hidden listener');const cancelCandidate=adapter.attachWindowPreview(started,value=>received.push(value));await new Promise(resolve=>setTimeout(resolve,20));
    assert.equal(received.at(-1).projection.errorCode,'preview_transport');assert.equal(calls.filter(call=>call.name==='application_preview_control'&&call.args.request.action==='stop').length,0,'A subscription error is not a cancel intent');cancelCandidate();await adapter.controlWindowPreview({leaseId:started.leaseId,generation:started.generation,action:'stop'});assert.equal(calls.filter(call=>call.name==='application_preview_control'&&call.args.request.action==='stop').length,1,'Explicit candidate owner cancellation still stops its own lease');
    const cancelSession=adapter.attachWindowPreview({...projection,leaseId:'native-adapter-session',consumerId:'session-owner'},value=>received.push(value));await new Promise(resolve=>setTimeout(resolve,20));assert.equal(calls.filter(call=>call.name==='application_preview_control'&&call.args.request.action==='stop').length,1,'Transport loss must not stop Rust-owned session capture');
    cancelCandidate();cancelSession();
    // A remounted view must request the retained frame even without a newer frame.
    failLatest=false;latestFrame={leaseId:'native-adapter-session',generation:'1',sequence:'7',targetIdentityHash:projection.targetIdentityHash,encoding:'jpeg',payload:[1,2,3]};
    const retainedProjection={...projection,leaseId:'native-adapter-session',state:'live'};
    for(let mount=0;mount<2;mount++){
      const reads=calls.filter(call=>call.name==='application_preview_latest').length;
      const unsubscribe=adapter.attachWindowPreview(retainedProjection,value=>received.push(value));await new Promise(resolve=>setTimeout(resolve,20));
      assert.equal(calls.filter(call=>call.name==='application_preview_latest').length,reads+1);assert.equal(calls.at(-1).args.afterSequence,null);assert.deepEqual(received.at(-1).frame,latestFrame);unsubscribe();
    }
    assert.equal(calls.filter(call=>call.name==='application_preview_control'&&call.args.request.action==='stop').length,1,'Reattaching the retained frame must not stop the Rust session');
    const p=await appPage();await p.evaluate(()=>{window.uiHarness.previewCapabilityFixture.permission='granted';});await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).first().click();await p.getByRole('radio').first().check();await p.waitForFunction(()=>window.uiHarness.previewRequests.length===1);
    await p.evaluate(()=>{const port=window.uiHarness,request=port.previewRequests[0],projection=[...port.previewProjections.values()].find(value=>value.consumerId===request.consumerId);port.emitPreview({projection:{...projection,state:'unavailable',errorCode:'preview_transport',detail:'Controlled transport failure'},frame:null});});
    await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),1);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.some(value=>value.action==='stop')),false,'Rendering/transport failure does not own producer shutdown');
    // A resume can change server generation before a cancelled view receives it.
    await p.evaluate(()=>{const port=window.uiHarness,request=port.previewRequests.at(-1),projection=[...port.previewProjections.values()].find(value=>value.consumerId===request.consumerId&&value.state!=='stopped');port.previewControlHolds.push({action:'resume',key:'late-auto-resume'});port.emitPreview({projection:{...projection,state:'permission_revoked',permission:'denied'},frame:null});});
    await p.getByRole('region',{name:'运行窗口预览',exact:true}).getByRole('button',{name:'刷新',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.previewControlPending.has('late-auto-resume'));await p.getByRole('button',{name:'取消',exact:true}).click();await p.evaluate(()=>window.uiHarness.releasePreviewControl('late-auto-resume'));
    await p.waitForFunction(()=>[...window.uiHarness.previewProjections.values()].filter(value=>value.leaseId.startsWith('ui-lease-')).every(value=>value.state==='stopped'));
    assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.equal(await p.getByRole('dialog').count(),0);await p.close();
  });
  await check('RBC-47-association-freezes-the-confirmed-target-until-receipt',async()=>{
    const p=await appPage();await p.evaluate(()=>{const port=window.uiHarness;port.publish({session:null});port.targetCandidates=[port.targetCandidates[0],{...port.targetCandidates[0],candidateId:'other-target',windowTitle:'Other controlled target',windowsPid:654}];port.holdAssociation=true;});
    await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).first().click();await p.getByRole('radio').first().check();await p.getByRole('button',{name:'关联并接入',exact:true}).click();await p.waitForFunction(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)===1&&window.uiHarness.associationResume!==null);
    assert.equal(await p.getByRole('radio').nth(1).isDisabled(),true);assert.equal(await p.getByRole('textbox',{name:'筛选窗口、程序或 Bottle',exact:true}).isDisabled(),true);assert.equal(await p.getByRole('button',{name:'进程列表',exact:true}).isDisabled(),true);
    await p.getByRole('radio').nth(1).evaluate(input=>input.click());assert.equal(await p.getByRole('radio').first().isChecked(),true);assert.equal(await p.getByRole('radio').nth(1).isChecked(),false);assert.equal(await p.getByRole('button',{name:'取消',exact:true}).isDisabled(),true);
    await p.evaluate(()=>window.uiHarness.associationResume());await p.getByRole('heading',{name:'正在接入游戏',exact:true}).waitFor();assert.equal(await p.evaluate(()=>window.uiHarness.associations[0].candidateId),await p.evaluate(()=>window.uiHarness.targetCandidates[0].candidateId));assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),1);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='select_source').length),0);await p.close();
  });
  await check('RBC-48-initial-scan-pending-strict-replay-and-real-single-flight',async()=>{
    // Exercise the production port, not a test-owned reimplementation of its cache.
    const nativeCalls=[],nativePending=[];
    const invoke=(name,args)=>{nativeCalls.push({name,args});assert.equal(name,'application_targets');return new Promise((resolve,reject)=>nativePending.push({resolve,reject}));};
    const stubs={'@tauri-apps/api/event':{listen:async()=>()=>{}},'@tauri-apps/api/core':{Channel:class{},invoke},'@tauri-apps/api/window':{getCurrentWindow:()=>({})},'@tauri-apps/api/webview':{getCurrentWebview:()=>({})}};
    const adapter=loadActualTauriPort(stubs);
    const first=adapter.targets(),replay=adapter.targets();assert.equal(first,replay);assert.equal(nativeCalls.length,1);
    const firstValue={candidates:[],diagnostics:[]};nativePending.shift().resolve(firstValue);assert.deepEqual(await Promise.all([first,replay]),[firstValue,firstValue]);
    const failed=adapter.targets(),failedReplay=adapter.targets();assert.equal(failed,failedReplay);assert.equal(nativeCalls.length,2);const rejected=Promise.allSettled([failed,failedReplay]);nativePending.shift().reject(Error('Controlled native scan failure'));assert.deepEqual((await rejected).map(item=>[item.status,item.reason.message]),[['rejected','Controlled native scan failure'],['rejected','Controlled native scan failure']]);
    const retried=adapter.targets(),retriedReplay=adapter.targets();assert.equal(retried,retriedReplay);assert.equal(nativeCalls.length,3);const retryValue={candidates:[],diagnostics:[]};nativePending.shift().resolve(retryValue);assert.deepEqual(await Promise.all([retried,retriedReplay]),[retryValue,retryValue]);

    // Production React disables StrictMode's effect replay. Keep the original
    // 47 checks on their production bundle and build this separate dev probe.
    const strictBuildCommand=[process.execPath,path.join(desktop,'node_modules/vite/bin/vite.js'),'build','--config',path.join(here,'vite.config.ts'),'--outDir',path.join(out,'strict-mode-build')];
    const build=spawn(strictBuildCommand[0],strictBuildCommand.slice(1),{cwd:desktop,env:{...process.env,NODE_ENV:'development'},stdio:['ignore','pipe','pipe']});let log='';build.stdout.on('data',data=>{log+=data;});build.stderr.on('data',data=>{log+=data;});const buildCode=await new Promise(resolve=>build.on('exit',resolve));await fs.writeFile(path.join(out,'strict-mode-build.log'),log);assert.equal(buildCode,0,log);
    const strictBuild={command:strictBuildCommand,environmentOverride:{NODE_ENV:'development'},sourceConfigSha256:hash(await fs.readFile(path.join(here,'vite.config.ts'))),logSha256:hash(Buffer.from(log)),exitCode:buildCode,scope:'separate_development_react_actual_effect_replay_controlled_transport',initialEffectAttachments:null};additionalTestBuilds.push(strictBuild);
    const listener=createServer();await new Promise(resolve=>listener.listen(0,'127.0.0.1',resolve));const strictPort=listener.address().port;await new Promise(resolve=>listener.close(resolve));
    strictChild=spawn(process.execPath,[path.join(desktop,'node_modules/vite/bin/vite.js'),'preview','--outDir',path.join(out,'strict-mode-build'),'--host','127.0.0.1','--port',String(strictPort),'--strictPort'],{cwd:desktop,stdio:'ignore'});
    const strictUrl=`http://127.0.0.1:${strictPort}/tests/visual/index.html?role=main&scenario=reading`;
    for(let attempt=0;attempt<100;attempt++){try{if((await fetch(strictUrl)).ok)break;}catch{}await new Promise(resolve=>setTimeout(resolve,100));}
    const captureProbe=async(strict,initialize)=>{
      const p=await browser.newPage({viewport:{width:1040,height:900},deviceScaleFactor:1});p.setDefaultTimeout(8000);p.on('pageerror',error=>errors.push(error.message));
      await p.addInitScript({content:`window.__controlledCaptureOnly=true;window.__controlledStrictMode=${strict};window.__initializeControlledPort=(${initialize.toString()});`});
      await navigateWithProductionCsp(p,strictUrl);await p.getByRole('dialog',{name:'选择正在运行的游戏',exact:true}).waitFor();return p;
    };
    const strict=await captureProbe(true,port=>{
      port.state.session=null;const target=port.targetCandidates[0];
      port.targetResponses=[{holdKey:'strict-old',value:{candidates:[{...target,candidateId:'strict-obsolete',windowTitle:'Obsolete strict setup'}],diagnostics:[]}},{holdKey:'strict-live',value:{candidates:[{...target,candidateId:'strict-live',windowTitle:'Current strict setup'}],diagnostics:[]}}];
    });
    await strict.waitForFunction(()=>window.uiHarness.targetRequests===2&&window.uiHarness.targetPending.has('strict-old')&&window.uiHarness.targetPending.has('strict-live'));strictBuild.initialEffectAttachments=await strict.evaluate(()=>window.uiHarness.targetRequests);assert.equal(strictBuild.initialEffectAttachments,2,'This probe must execute actual StrictMode setup-cleanup-setup');
    const dialog=strict.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await dialog.getByText('正在扫描运行中的游戏…扫描完成后可选择目标。',{exact:true}).waitFor();assert.equal(await dialog.getByRole('radio').count(),0);assert.equal(await dialog.getByRole('button',{name:'扫描进程',exact:true}).isDisabled(),true);assert.equal(await dialog.getByRole('button',{name:'关联并接入',exact:true}).isDisabled(),true);assert.equal(await dialog.getByRole('button',{name:'取消',exact:true}).isEnabled(),true);
    await strict.evaluate(()=>window.uiHarness.releaseTargets('strict-old'));await strict.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await dialog.getByRole('radio').count(),0);assert.equal(await dialog.getByText('Obsolete strict setup',{exact:true}).count(),0);assert.equal(await dialog.getByRole('button',{name:'扫描进程',exact:true}).isDisabled(),true);
    await strict.evaluate(()=>window.uiHarness.releaseTargets('strict-live'));await dialog.getByRole('radio',{name:/Current strict setup/}).waitFor();assert.equal(await dialog.getByRole('radio').count(),1);assert.equal(await strict.evaluate(()=>window.uiHarness.targetRequests),2);assert.equal(await strict.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);await screenshot(strict,'capture-initial-strict-replay-current-only');await strict.close();

    const cancelled=await captureProbe(false,port=>{port.state.session=null;port.targetResponses=[{holdKey:'closed-scan',value:{candidates:[{...port.targetCandidates[0],candidateId:'closed-scan',windowTitle:'Late closed scan'}],diagnostics:[]}}];});
    await cancelled.waitForFunction(()=>window.uiHarness.targetPending.has('closed-scan'));await cancelled.getByRole('button',{name:'取消',exact:true}).click();assert.equal(await cancelled.getByRole('dialog').count(),0);
    await cancelled.evaluate(()=>{const port=window.uiHarness;port.targetResponses.push({holdKey:'reopened-scan',value:{candidates:[{...port.targetCandidates[0],candidateId:'reopened-scan',windowTitle:'Current reopened scan'}],diagnostics:[]}});});await cancelled.getByRole('button',{name:'重新打开关联',exact:true}).click();await cancelled.waitForFunction(()=>window.uiHarness.targetRequests===2&&window.uiHarness.targetPending.has('reopened-scan'));
    await cancelled.evaluate(()=>window.uiHarness.releaseTargets('closed-scan'));await cancelled.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await cancelled.getByRole('radio').count(),0);assert.equal(await cancelled.getByText('Late closed scan',{exact:true}).count(),0);assert.equal(await cancelled.getByRole('button',{name:'扫描进程',exact:true}).isDisabled(),true);
    await cancelled.evaluate(()=>window.uiHarness.releaseTargets('reopened-scan'));await cancelled.getByRole('radio',{name:/Current reopened scan/}).waitFor();assert.equal(await cancelled.getByRole('radio').count(),1);assert.equal(await cancelled.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.equal(await cancelled.evaluate(()=>window.uiHarness.targetRequests),2);await cancelled.close();
    for(const mode of ['empty','error']){
      const p=await captureProbe(false,mode==='empty'?port=>{port.state.session=null;port.targetResponses=[{value:{candidates:[],diagnostics:[]}}];}:port=>{port.state.session=null;port.targetResponses=[{error:'Controlled initial scan failure'}];});
      await p.getByText(mode==='empty'?'没有发现运行中的游戏':'扫描失败，可以重试',{exact:true}).waitFor();assert.equal(await p.getByRole('radio').count(),0);assert.equal(await p.getByRole('button',{name:'关联并接入',exact:true}).isDisabled(),true);assert.equal(await p.getByRole('button',{name:'扫描进程',exact:true}).isEnabled(),true);if(mode==='error')assert.equal(await p.getByText('Controlled initial scan failure',{exact:true}).count(),1);
      await p.evaluate(()=>{const port=window.uiHarness;port.targetResponses.push({holdKey:'recovered-scan',value:{candidates:[{...port.targetCandidates[0],windowTitle:'Recovered scan result'}],diagnostics:[]}});});await p.getByRole('button',{name:'扫描进程',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.targetPending.has('recovered-scan'));assert.equal(await p.getByRole('button',{name:'扫描进程',exact:true}).isDisabled(),true);assert.equal(await p.getByRole('textbox',{name:'筛选窗口、程序或 Bottle',exact:true}).isDisabled(),true);
      await p.evaluate(()=>window.uiHarness.releaseTargets('recovered-scan'));await p.getByRole('radio',{name:/Recovered scan result/}).waitFor();assert.equal(await p.evaluate(()=>window.uiHarness.targetRequests),2);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);await p.close();
    }
  });
  await check('RBC-49-existing-library-association-is-visible-and-target-scoped',async()=>{
    const setup=port=>{
      port.state.session=null;port.state.current=null;port.state.history.units=[];const base=port.targetCandidates[0];
      port.targetCandidates=['A','B'].map((label,index)=>({...base,candidateId:`mapped-target-${label}`,windowTitle:`Controlled target ${label}`,executablePath:base.executablePath.replace(/Game\.exe$/,`${label}.exe`),windowsPid:1001+index}));
      port.state.games=port.targetCandidates.map((target,index)=>({gameId:`mapped-game-${index?'B':'A'}`,displayName:`Controlled library ${index?'B':'A'}`,originalTitle:null,sourceLanguage:index?'zh-CN':'ja',coverAssetRef:null,rowRevision:1,target:{environmentKind:'crossover',environmentKey:target.environmentKey,bottleLocator:'/UI-test/Bottle',launchPath:target.executablePath,canonicalExePath:target.executablePath,targetPathKey:null,workingDirectory:null,arguments:[],architecture:target.architecture,verificationState:'verified'}}));
      port.bindingFixture={...port.bindingFixture,state:'unbound',sessionId:null,targetIdentitySha256:null};port.bindingJourneyFixture.autoMatchGameId='mapped-game-B';
    };
    const p=await appPage('main',1040,900,setup);assert.equal(await p.evaluate(()=>window.uiHarness.state.games[0].target.environmentKey===window.uiHarness.state.games[1].target.environmentKey),true);assert.notEqual(await p.evaluate(()=>window.uiHarness.state.games[0].target.canonicalExePath),await p.evaluate(()=>window.uiHarness.state.games[1].target.canonicalExePath));
    await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).click();let dialog=p.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await dialog.getByRole('radio',{name:/Controlled target B/}).check();let mapping=dialog.getByRole('combobox',{name:'关联到游戏库',exact:true});assert.equal(await mapping.getAttribute('data-select-value'),'game:mapped-game-B','Global entry must select the exact target, never the first library game');assert.match(await mapping.textContent(),/Controlled library B/);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);await dialog.getByRole('button',{name:'取消',exact:true}).click();
    await p.getByRole('button',{name:/Controlled library A/}).click();await p.evaluate(()=>{const port=window.uiHarness;port.publish({session:{sessionId:'controlled-scoped-A',gameId:'mapped-game-A',sourceKind:'hook',state:'stopped',paused:false,translationEnabled:true,sources:[]}})});await p.getByRole('button',{name:'当前会话',exact:true}).click();await p.getByRole('button',{name:'重新连接游戏…',exact:true}).click();dialog=p.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await dialog.getByRole('radio',{name:/Controlled target B/}).check();mapping=dialog.getByRole('combobox',{name:'关联到游戏库',exact:true});assert.equal(await mapping.getAttribute('data-select-value'),'game:mapped-game-A');await dialog.getByText('完整程序路径与游戏资料不一致，请核对实际游戏，不能把其他程序关联到这款游戏。',{exact:true}).waitFor();const confirm=dialog.getByRole('button',{name:'关联并接入',exact:true});assert.equal(await confirm.isDisabled(),true);await confirm.evaluate(button=>button.click());assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games').length),0);
    await mapping.click();await p.getByRole('option',{name:'Controlled library B',exact:true}).click();assert.equal(await mapping.getAttribute('data-select-value'),'game:mapped-game-B');assert.equal(await confirm.isEnabled(),true);await screenshot(p,'capture-existing-visible-target-scoped-mapping');await confirm.click();await p.getByRole('dialog',{name:'正在接入游戏',exact:true}).waitFor();assert.deepEqual(await p.evaluate(()=>window.uiHarness.reviewedAssociations.map(({review})=>({gameId:review.gameId,candidateId:review.candidate.candidateId}))),[{gameId:'mapped-game-B',candidateId:'mapped-target-B'}]);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games').length),0);assert.equal(await p.evaluate(()=>window.uiHarness.state.session.gameId),'mapped-game-B');await p.close();
    const session=await appPage('main',1040,900,setup);await session.getByRole('button',{name:'连接运行中的游戏…',exact:true}).click();const only=session.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await only.getByRole('radio',{name:/Controlled target B/}).check();const choice=only.getByRole('combobox',{name:'关联到游戏库',exact:true});await choice.click();await session.getByRole('option',{name:'仅本次连接，不加入游戏库',exact:true}).click();assert.equal(await choice.getAttribute('data-select-value'),'session');await only.getByText('只建立本次取文会话，不保存游戏资料或游戏库封面。',{exact:true}).waitFor();await only.getByRole('button',{name:'关联并接入',exact:true}).click();await session.getByRole('dialog',{name:'正在接入游戏',exact:true}).waitFor();assert.deepEqual(await session.evaluate(()=>window.uiHarness.reviewedAssociations.map(({review})=>({gameId:review.gameId,candidateId:review.candidate.candidateId}))),[{gameId:null,candidateId:'mapped-target-B'}]);assert.equal(await session.evaluate(()=>window.uiHarness.state.games.length),2);assert.equal(await session.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games').length),0);assert.equal(await session.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='select_source').length),0);await session.close();
  });
  await check('RBC-50-new-record-typed-save-failure-and-retry-do-not-misassociate',async()=>{
    const p=await appPage('main',1040,900,port=>{port.state.session=null;port.state.current=null;port.state.history.units=[];port.state.games=[];port.targetCandidates=[port.targetCandidates[0],{...port.targetCandidates[0],candidateId:'other-new-target',windowTitle:'Other new target',windowsPid:999}];port.bindingFixture={...port.bindingFixture,state:'unbound',sessionId:null,targetIdentitySha256:null};port.saveGamesResponses.push({games:[],gameResults:[],error:'Controlled game save failure'});});
    await p.getByRole('button',{name:'连接运行中的游戏…',exact:true}).click();const dialog=p.getByRole('dialog',{name:'选择正在运行的游戏',exact:true});await dialog.getByRole('radio').first().check();const mapping=dialog.getByRole('combobox',{name:'关联到游戏库',exact:true});assert.equal(await mapping.getAttribute('data-select-value'),'new');const name=dialog.getByRole('textbox',{name:'游戏名称 *',exact:true}),language=dialog.getByRole('combobox',{name:'原文语言',exact:true}),confirm=dialog.getByRole('button',{name:'关联并接入',exact:true});assert.equal(await language.getAttribute('data-select-value'),'');await name.fill('  Controlled saved game 日本語  ');await language.click();await p.getByRole('option',{name:'日语',exact:true}).click();await confirm.click();await dialog.getByText('Controlled game save failure',{exact:false}).waitFor();assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.equal(await p.evaluate(()=>window.uiHarness.state.games.length),0);assert.equal(await name.inputValue(),'  Controlled saved game 日本語  ');assert.equal(await language.getAttribute('data-select-value'),'ja');
    const initial=await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games')[0].command.payload.items[0]);assert.ok(initial.itemId);assert.equal(initial.game.displayName,'Controlled saved game 日本語');assert.equal(initial.game.sourceLanguage,'ja');assert.equal(initial.game.gameId,null);assert.equal(initial.game.target.canonicalExePath,await p.evaluate(()=>window.uiHarness.targetCandidates[0].executablePath));
    // Reject a typed response whose itemId belongs to another request. Construct
    // it from the actual command so no production save identity is hardcoded.
    await p.evaluate(()=>window.uiHarness.saveGamesHolds.push('wrong-save-receipt'));await confirm.click();await p.waitForFunction(()=>window.uiHarness.saveGamesPending.has('wrong-save-receipt'));await p.evaluate(()=>{const port=window.uiHarness,item=port.commands.filter(value=>value.command.kind==='save_games').at(-1).command.payload.items[0];port.saveGamesResponses.push({games:[],gameResults:[{itemId:`other-${item.itemId}`,gameId:crypto.randomUUID(),rowRevision:1,reused:false}]});port.releaseSaveGames('wrong-save-receipt');});await dialog.getByText('保存结果无法对应本次游戏资料，未发起 Hook 接入。请核对后重试。',{exact:false}).waitFor();assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);assert.equal(await mapping.getAttribute('data-select-value'),'new');
    await p.evaluate(()=>window.uiHarness.saveGamesHolds.push('correct-save-receipt'));await confirm.evaluate(button=>{button.click();button.click();});await p.waitForFunction(()=>window.uiHarness.saveGamesPending.has('correct-save-receipt'));assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games').length),3);assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);
    for(const control of [name,language,mapping,dialog.getByRole('radio').nth(1),dialog.getByRole('button',{name:'扫描进程',exact:true}),dialog.getByRole('button',{name:'取消',exact:true})])assert.equal(await control.isDisabled(),true);await dialog.getByRole('radio').nth(1).evaluate(input=>input.click());assert.equal(await dialog.getByRole('radio').first().isChecked(),true);assert.equal(await dialog.getByRole('radio').nth(1).isChecked(),false);await confirm.evaluate(button=>{button.click();button.click();});assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games').length),3);
    const saved=await p.evaluate(()=>{const port=window.uiHarness,item=port.commands.filter(value=>value.command.kind==='save_games').at(-1).command.payload.items[0],gameId=crypto.randomUUID(),game={...structuredClone(item.game),gameId,rowRevision:1};port.saveGamesResponses.push({games:[game],gameResults:[{itemId:item.itemId,gameId,rowRevision:1,reused:false}]});port.bindingJourneyFixture.autoMatchGameId=gameId;port.associationResponses.push({error:'Controlled Hook connection failure'});port.releaseSaveGames('correct-save-receipt');return {itemId:item.itemId,gameId,game};});assert.equal(saved.itemId,initial.itemId);
    await dialog.getByText('Controlled Hook connection failure',{exact:false}).waitFor();assert.equal(await mapping.getAttribute('data-select-value'),`game:${saved.gameId}`);assert.match(await mapping.textContent(),/Controlled saved game 日本語/);await dialog.getByText('游戏资料已保存；如接入失败，重试将复用这条记录。',{exact:true}).waitFor();assert.equal(await p.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),1);assert.equal(await p.evaluate(()=>window.uiHarness.reviewedAssociations[0].review.gameId),saved.gameId);assert.equal(await p.evaluate(()=>window.uiHarness.state.games.length),1);assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.games[0]),saved.game);await screenshot(p,'capture-save-typed-receipt-hook-failure-retry');
    await dialog.getByRole('region',{name:'上次接入失败',exact:true}).getByRole('button',{name:'重试接入',exact:true}).click();await p.getByRole('dialog',{name:'正在接入游戏',exact:true}).waitFor();assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='save_games').length),3,'Hook retry must reuse the actual saved game rather than save a second record');assert.deepEqual(await p.evaluate(()=>window.uiHarness.reviewedAssociations.map(({review})=>({gameId:review.gameId,candidateId:review.candidate.candidateId}))),[{gameId:saved.gameId,candidateId:'ui-target'},{gameId:saved.gameId,candidateId:'ui-target'}]);assert.equal(await p.evaluate(()=>window.uiHarness.commands.filter(item=>item.command.kind==='select_source').length),0);assert.equal(await p.evaluate(()=>window.uiHarness.state.session.gameId),saved.gameId);await p.close();
  });
  const expiredHistory = {code:'snapshot_expired',stage:'session_records',message:'查看范围已过期，请回到最新范围。',recoverable:true};
  const historyPlan = (p,plan) => p.evaluate(plan=>window.uiHarness.historyControl.plans.push(plan),plan);
  const appendHistory = (p,sequence) => p.evaluate(sequence=>{const port=window.uiHarness,rows=port.sessionRecordFixtures[`${port.state.session.sessionId}:body`],last=structuredClone(rows.at(-1));rows.push({...last,sequence:String(sequence),unit:{...last.unit,unitId:`history-unit-${sequence}`,sourceText:`原文 ${sequence}`,sourceHash:String(sequence).padStart(64,'0')}});},sequence);
  const freshHistoryReads = p => p.evaluate(()=>window.uiHarness.sessionRecordQueries.filter(query=>query.dataset==='body'&&query.cursor===null).length);
  const settleHistory = p => p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await check('RBC-51-follow-expired-opens-one-new-range',async()=>{
    const p=await historyPage(3),before=await p.evaluate(()=>window.uiHarness.historyControl.receipts.find(receipt=>receipt.query.dataset==='body').snapshot);await appendHistory(p,4);await historyPlan(p,{cursorOnly:true,typedError:expiredHistory});
    await historyRow(p,4).waitFor();const after=await p.evaluate(()=>window.uiHarness.historyControl.receipts.at(-1).snapshot);assert.notEqual(after.snapshotId,before.snapshotId);assert.equal((await historyState(p)).highWatermark,'4');assert.equal(await freshHistoryReads(p),2);assert.equal(await p.getByRole('button',{name:'跟随新内容',exact:true}).count(),1);assert.equal(await p.getByRole('alert').count(),0);await screenshot(p,'history-expired-follow-recovers-new-range');await p.close();
  });
  await check('RBC-52-paused-expired-keeps-pages-and-anchor-until-latest',async()=>{
    const p=await historyPage(160);await loadHistoryOlder(p,100);await pauseHistory(p);const list=p.getByRole('log',{name:'正文历史列表',exact:true});await list.evaluate(node=>{const row=node.querySelector('[data-record-sequence="100"]');node.scrollTop+=row.getBoundingClientRect().top-node.getBoundingClientRect().top+9;});const before=await historyState(p),anchor=await visibleHistoryAnchor(p);await appendHistory(p,161);await historyPlan(p,{cursorOnly:true,typedError:expiredHistory});await p.getByRole('alert').filter({hasText:'查看范围已过期'}).waitFor();assert.deepEqual(await historyState(p),before);assert.deepEqual(await visibleHistoryAnchor(p),anchor);assert.equal(await freshHistoryReads(p),1);await p.getByRole('button',{name:'回到最新范围',exact:true}).click();await historyRow(p,161).waitFor();assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);assert.equal(await freshHistoryReads(p),2);await p.close();
  });
  await check('RBC-53-expiry-recovery-failure-retains-rows-and-retries-latest',async()=>{
    const p=await historyPage(3),before=await historyState(p);await appendHistory(p,4);await historyPlan(p,{cursorOnly:true,typedError:expiredHistory});await historyPlan(p,{latestOnly:true,error:'Controlled latest recovery offline'});await p.getByRole('alert').filter({hasText:'Controlled latest recovery offline'}).waitFor();assert.deepEqual(await historyState(p),before);assert.equal(await freshHistoryReads(p),2);const count=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await p.waitForTimeout(2200);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length),count);await p.getByRole('button',{name:'重试获取最新范围',exact:true}).click();await historyRow(p,4).waitFor();assert.equal(await freshHistoryReads(p),3);assert.equal(await p.getByRole('alert').count(),0);await p.close();
  });
  await check('RBC-54-late-expiry-old-filter-session-dataset-is-ignored',async()=>{
    for(const change of ['filter','session','dataset'])for(const waitAt of ['frozen','recovery']){
      const p=await historyPage(3);await historyPlan(p,{cursorOnly:true,...(waitAt==='frozen'?{holdKey:'old-expiry'}:{}),typedError:expiredHistory});if(waitAt==='recovery')await historyPlan(p,{latestOnly:true,holdKey:'old-expiry'});await p.waitForFunction(()=>window.uiHarness.historyControl.pending.has('old-expiry'));const before=await freshHistoryReads(p);
      if(change==='filter'){await p.getByRole('textbox',{name:'搜索会话原文',exact:true}).fill('原文 2');await p.waitForFunction(()=>document.querySelectorAll('details[data-flat-session-section][open] [data-record-sequence]').length===1);}
      else if(change==='session'){await p.evaluate(()=>{const port=window.uiHarness,id='fresh-history-after-expiry',row=structuredClone(port.sessionRecordFixtures[`${port.state.session.sessionId}:body`][0]);row.sequence='901';row.unit={...row.unit,unitId:'replacement-history-unit',sessionId:id,sourceText:'新会话原文'};port.sessionRecordFixtures[`${id}:body`]=[row];port.bindingFixture={...port.bindingFixture,sessionId:id};port.publish({session:{...port.state.session,sessionId:id}});});await openSessionDataset(p,'正文历史');await historyRow(p,901).waitFor();}
      else {await openSessionDataset(p,'全部提取');await p.getByRole('log',{name:'全量提取文本列表',exact:true}).waitFor();}
      const changed=await p.locator('details[data-flat-session-section][open] [data-record-sequence]').allTextContents();const reads=await freshHistoryReads(p);await releaseHistory(p,'old-expiry');await p.waitForFunction(()=>[...window.uiHarness.historyControl.active.values()].every(count=>count===0));await settleHistory(p);assert.deepEqual(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').allTextContents(),changed,'A late body response cannot change the active dataset');assert.equal(await freshHistoryReads(p),reads+(change==='dataset'&&waitAt==='frozen'?1:0),'Only its already-started same-scope expiry recovery may finish while collapsed');assert.equal(await p.getByRole('alert').count(),0);assert.equal(reads,before+(change==='dataset'?0:1));if(change==='dataset')assert.equal(await p.locator('[data-session-dataset="body"] [role="log"]').count(),1,'Collapsing keeps the body query owner mounted');await p.close();
    }
  });
  await check('RBC-55-pause-during-expiry-recovery-discards-new-range',async()=>{
    const p=await historyPage(3),before=await historyState(p);await appendHistory(p,4);await historyPlan(p,{cursorOnly:true,typedError:expiredHistory});await historyPlan(p,{latestOnly:true,holdKey:'expiry-latest'});await p.waitForFunction(()=>window.uiHarness.historyControl.pending.has('expiry-latest'));await pauseHistory(p);await releaseHistory(p,'expiry-latest');await p.getByRole('alert').filter({hasText:'查看范围已过期'}).waitFor();assert.deepEqual(await historyState(p),before);assert.equal(await historyRow(p,4).count(),0);assert.equal(await p.getByRole('button',{name:'跟随新内容',exact:true}).count(),0);await p.getByRole('button',{name:'回到最新范围',exact:true}).click();await historyRow(p,4).waitFor();assert.equal(await freshHistoryReads(p),3);await p.close();
  });
  await check('RBC-56-nonexpiry-and-wrong-stage-errors-never-reset-range',async()=>{
    for(const plan of [{error:''},{typedError:{code:expiredHistory.code,stage:expiredHistory.stage,message:expiredHistory.message}},{error:expiredHistory.message},{typedError:{...expiredHistory,stage:'other'}},{typedError:{...expiredHistory,code:'network_unavailable'}},{typedError:{...expiredHistory,recoverable:false}},{typedError:{...expiredHistory,code:'invalid_cursor',stage:'other'}}]){
      const p=await historyPage(3),before=await historyState(p);await historyPlan(p,{cursorOnly:true,...plan});await p.getByRole('alert').waitFor();assert.deepEqual(await historyState(p),before);assert.equal(await freshHistoryReads(p),1);if(plan.error===''){const count=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await p.waitForTimeout(2200);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length),count);}const failed=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.at(-1).cursor);await p.getByRole('button',{name:'重试',exact:true}).click();await p.getByRole('alert').waitFor({state:'hidden'});assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.at(-1).cursor),failed);assert.equal(await freshHistoryReads(p),1);await p.close();
    }
  });
  await check('RBC-57-invalid-cursor-requires-explicit-latest',async()=>{
    for(const fault of [{code:'invalid_cursor',message:'范围游标已失效（模拟已被GC回收）'},{code:'cursor_range_changed',message:'范围游标已失效（模拟清理）'},{code:'invalid_cursor',message:''}]){
      const p=await historyPage(3),before=await historyState(p);await appendHistory(p,4);await historyPlan(p,{cursorOnly:true,typedError:{...expiredHistory,...fault}});await p.getByRole('alert').waitFor();assert.deepEqual(await historyState(p),before);assert.equal(await freshHistoryReads(p),1);assert.equal(await p.getByRole('button',{name:'重试',exact:true}).count(),0);const count=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await p.waitForTimeout(2200);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length),count);await p.getByRole('button',{name:'回到最新范围',exact:true}).click();await historyRow(p,4).waitFor();assert.equal(await freshHistoryReads(p),2);await p.close();
    }
  });
  await check('RBC-58-expiry-with-queued-navigation-preserves-priority',async()=>{
    const p=await historyPage(160);await pauseHistory(p);await historyPlan(p,{cursorOnly:true,holdKey:'expiry-queue',typedError:expiredHistory});await p.waitForFunction(()=>window.uiHarness.historyControl.pending.has('expiry-queue'));await p.getByRole('button',{name:'加载较早记录',exact:true}).click();await appendHistory(p,161);await p.getByRole('button',{name:/^回到最新/}).click();const before=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await releaseHistory(p,'expiry-queue');await historyRow(p,161).waitFor();assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);assert.equal((await historyState(p)).highWatermark,'161');const tail=await p.evaluate(before=>window.uiHarness.sessionRecordQueries.slice(before).map(query=>({...query,offset:query.cursor?window.uiHarness.sessionRecordCursors.get(query.cursor)?.offset:null})),before);assert.equal(tail.filter(query=>query.cursor===null).length,1);assert.equal(tail.some(query=>query.offset===50),false);assert.equal(await p.getByRole('alert').count(),0);assert.equal(await p.evaluate(()=>Math.max(...window.uiHarness.historyControl.maxActive.values())),1);await p.close();
  });
  await check('RBC-59-expiry-recovery-single-flight',async()=>{
    const p=await historyPage(3);await appendHistory(p,4);await historyPlan(p,{cursorOnly:true,typedError:expiredHistory});await historyPlan(p,{latestOnly:true,holdKey:'single-latest'});await p.waitForFunction(()=>window.uiHarness.historyControl.pending.has('single-latest'));const count=await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length);await p.waitForTimeout(2200);assert.equal(await p.evaluate(()=>window.uiHarness.sessionRecordQueries.length),count);assert.equal(await p.evaluate(()=>Math.max(...window.uiHarness.historyControl.maxActive.values())),1);await releaseHistory(p,'single-latest');await historyRow(p,4).waitFor();assert.equal(await freshHistoryReads(p),2);await p.close();
  });
  await check('RBC-60-older-expiry-keeps-old-range-until-latest',async()=>{
    const p=await historyPage(160);await pauseHistory(p);await holdHistoryRefresh(p,'older-expiry-before-read');const before=await historyState(p);await historyPlan(p,{cursorOnly:true,offset:50,typedError:expiredHistory});await p.getByRole('button',{name:'加载较早记录',exact:true}).click();await releaseHistory(p,'older-expiry-before-read');await p.getByRole('alert').filter({hasText:'查看范围已过期'}).waitFor();assert.deepEqual(await historyState(p),before);assert.equal(await freshHistoryReads(p),1);await p.getByRole('button',{name:'回到最新范围',exact:true}).click();await p.getByRole('alert').waitFor({state:'hidden'});assert.equal(await freshHistoryReads(p),2);assert.equal(await p.locator('details[data-flat-session-section][open] [data-record-sequence]').count(),50);await p.close();
  });
  // The transport only delivers controlled replies/events. Real WordCard and
  // WordCardSurface own frontend lifecycle; none of these facts are OS proof.
  function initializeWordCardTransport(port) {
    const open = port.openWordCard.bind(port), close = port.closeWordCard.bind(port), subscribe = port.onWordCardClosed.bind(port);
    const listeners = new Set(), held = new Map();
    port.wordCardProbe = { closeRequests: [], openReplies: [], holds: [], held, closedEvents: [], listeners,
      emitClosed(id) { this.closedEvents.push(id); for (const listener of [...listeners]) listener(id); },
      release(key) { const resolve = held.get(key); if (!resolve) throw Error(`Missing controlled open reply: ${key}`); held.delete(key); resolve(); },
    };
    port.onWordCardClosed = async listener => { const dispose = await subscribe(listener); listeners.add(listener); return () => { listeners.delete(listener); dispose(); }; };
    port.openWordCard = async request => { const hold = port.wordCardProbe.holds.shift(), id = await open(request); port.wordCardProbe.openReplies.push(id); if (hold) await new Promise(resolve => held.set(hold, resolve)); return id; };
    port.closeWordCard = async id => { port.wordCardProbe.closeRequests.push(id); await close(id); };
    port.dictionaryEntries = [{ entryId: 'wordcard-fixed-entry', lemma: '放課後', reading: 'ほうかご', senses: [{ language: 'zh', text: '固定释义：放学以后。', usageNotes: null }], sourceLabel: '组件输入词典', sourceVersion: 'controlled-v1', licenseRef: 'component-fixture', attribution: 'controlled dictionary input; not a real dictionary resource' }];
  }
  const wordCardPage = async () => appPage('hud', 700, 900, initializeWordCardTransport);
  const openWordCard = async p => {
    await p.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).first().evaluate(node => { window.controlledOriginalWordCardAnchor = node; });
    await p.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).first().click();
    await p.getByRole('dialog', { name: '本地词典', exact: true }).getByText('固定释义：放学以后。', { exact: true }).waitFor();
    await p.waitForFunction(() => window.uiHarness.wordCardProbe.openReplies.length > 0);
    return p.evaluate(() => ({ card: structuredClone(window.uiHarness.wordCard), rect: structuredClone(window.uiHarness.wordCardRect) }));
  };
  const assertFrozenWordCard = async (p, frozen) => {
    assert.deepEqual(await p.evaluate(() => ({ card: structuredClone(window.uiHarness.wordCard), rect: structuredClone(window.uiHarness.wordCardRect) })), frozen);
    assert.equal(await p.getByRole('dialog', { name: '本地词典', exact: true }).isVisible(), true);
    assert.equal(await p.getByRole('dialog', { name: '本地词典', exact: true }).getByText('固定释义：放学以后。', { exact: true }).count(), 1);
  };
  await check('RBC-61-wordcard-content-updates-and-owner-resize-preserve-frozen-object', async () => {
    const p = await wordCardPage(), frozen = await openWordCard(p);
    const initialFits = await p.evaluate(() => window.uiHarness.hudFitRequests.length);
    for (const [index, repetitions] of [4, 1, 8].entries()) {
      const id = `controlled-new-length-${index}`;
      await p.evaluate(async ({ id, repetitions }) => {
        const port = window.uiHarness, base = structuredClone(port.wordCard.unit);
        let bytes = 0, utf16 = 0;
        const tokens = Array.from({ length: repetitions }, () => [['言葉', 'ことば', '名詞'], ['を', '', '助詞'], ['読む', 'よむ', '動詞'], ['。', '', '補助記号']].map(([surface, reading, partOfSpeech]) => {
          const byteStart = bytes, utf16Start = utf16; bytes += new TextEncoder().encode(surface).length; utf16 += surface.length;
          return { surface, reading, lemma: surface, partOfSpeech, byteStart, byteEnd: bytes, utf16Start, utf16End: utf16, known: true };
        })).flat();
        const sourceText = tokens.map(token => token.surface).join(''), sourceHash = [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(sourceText)))].map(value => value.toString(16).padStart(2, '0')).join('');
        port.wordCardProbe.pendingTokens = tokens;
        // The new source arrives before its lexical reply. This really removes
        // the old DOM anchor, rather than just changing the same token's text.
        const next = { ...base, unitId: id, sourceText, sourceHash, tokens: [], translations: [], createdAtMs: base.createdAtMs + 1000 * (port.state.history.units.length + 1) };
        port.publish({ current: next, history: { units: [next, ...port.state.history.units], nextCursor: null } });
      }, { id, repetitions });
      await p.waitForFunction(id => window.uiHarness.state.current.unitId === id && document.querySelector('[data-testid="source-text"]')?.textContent.includes('言葉'), id);
      assert.equal(await p.evaluate(() => window.controlledOriginalWordCardAnchor.isConnected), false);
      assert.equal(await p.getByTestId('source-text').getByRole('button').count(), 0);
      await p.evaluate(() => window.dispatchEvent(new Event('resize'))); await assertFrozenWordCard(p, frozen);
      await p.evaluate(() => { const port = window.uiHarness, next = { ...port.state.current, tokens: port.wordCardProbe.pendingTokens }; port.publish({ current: next, history: { units: [next, ...port.state.history.units.filter(unit => unit.unitId !== next.unitId)], nextCursor: null } }); });
      await p.getByTestId('source-text').getByRole('button', { name: '言葉，读音 ことば', exact: true }).first().waitFor();
      assert.equal(await p.getByTestId('source-text').getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).count(), 0);
      await assertFrozenWordCard(p, frozen);
      for (const showRuby of [false, true]) {
        await p.evaluate(showRuby => { const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, showRuby } }); }, showRuby);
        await p.waitForFunction(showRuby => document.querySelector('[data-testid="source-text"]')?.getAttribute('data-ruby') === String(showRuby), showRuby);
        assert.equal(await p.getByTestId('source-text').locator('rt').count() > 0, showRuby);
        await p.evaluate(() => window.dispatchEvent(new Event('resize')));
        await assertFrozenWordCard(p, frozen);
      }
      for (const text of ['译文', '译文逐步到达，内容增加。'.repeat(index + 2), '本句译文最终完成。'.repeat(index + 1)]) {
        await p.evaluate(text => { const port = window.uiHarness; port.publish({ current: { ...port.state.current, translations: [{ translationId: `translation-${port.state.current.unitId}`, taskId: `task-${port.state.current.unitId}`, providerLabel: 'controlled streaming projection', targetLanguage: 'zh-CN', correction: null, text }] } }); }, text);
        await p.getByRole('article').filter({ hasText: text }).waitFor();
        await p.evaluate(() => window.dispatchEvent(new Event('resize')));
        await assertFrozenWordCard(p, frozen);
      }
    }
    await p.waitForFunction(count => window.uiHarness.hudFitRequests.length > count, initialFits);
    assert.equal(await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length), 0);
    assert.equal(await p.evaluate(() => window.uiHarness.wordCardRequests.length), 1);
    assert.equal(await p.evaluate(() => window.uiHarness.state.current.unitId), 'controlled-new-length-2');
    assert.equal(await p.evaluate(() => window.uiHarness.wordCard.unit.unitId), frozen.card.unit.unitId);
    await screenshot(p, 'wordcard-frozen-through-new-length-ruby-stream-resize'); await p.close();
  });
  await check('RBC-62-exact-native-closed-event-isolates-new-card', async () => {
    const p = await wordCardPage(), first = await openWordCard(p), dialog = p.getByRole('dialog', { name: '本地词典', exact: true });
    await p.evaluate(id => window.uiHarness.wordCardProbe.emitClosed(id), first.card.cardId); await dialog.waitFor({ state: 'hidden' });
    assert.equal(await p.evaluate(() => window.uiHarness.wordCard), null);
    const second = await openWordCard(p); assert.notEqual(second.card.cardId, first.card.cardId);
    const closes = await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length);
    for (const id of [first.card.cardId, 'unknown-native-old-card']) { await p.evaluate(id => window.uiHarness.wordCardProbe.emitClosed(id), id); await assertFrozenWordCard(p, second); }
    assert.equal(await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length), closes);
    await p.evaluate(id => window.uiHarness.wordCardProbe.emitClosed(id), second.card.cardId); await dialog.waitFor({ state: 'hidden' });
    assert.equal(await p.evaluate(() => window.uiHarness.wordCard), null); await p.close();
  });
  await check('RBC-63-escape-and-owner-outside-pointer-close-without-blur-close', async () => {
    const p = await wordCardPage(), first = await openWordCard(p), dialog = p.getByRole('dialog', { name: '本地词典', exact: true });
    await p.evaluate(() => { window.dispatchEvent(new Event('blur')); document.dispatchEvent(new Event('visibilitychange')); }); await assertFrozenWordCard(p, first);
    await dialog.dispatchEvent('pointerdown', { bubbles: true, button: 0 }); await assertFrozenWordCard(p, first);
    await p.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).first().dispatchEvent('pointerdown', { bubbles: true, button: 0 }); await assertFrozenWordCard(p, first);
    assert.equal(await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length), 0);
    await p.keyboard.press('Escape'); await dialog.waitFor({ state: 'hidden' });
    assert.equal(await p.evaluate(id => window.uiHarness.wordCardProbe.closeRequests.includes(id), first.card.cardId), true);
    const second = await openWordCard(p); await p.locator('body').dispatchEvent('pointerdown', { bubbles: true, button: 0 }); await dialog.waitFor({ state: 'hidden' });
    assert.equal(await p.evaluate(id => window.uiHarness.wordCardProbe.closeRequests.includes(id), second.card.cardId), true); assert.equal(await p.evaluate(() => window.uiHarness.wordCard), null); await p.close();
  });
  await check('RBC-64-owner-scroll-closes-but-surface-and-unrelated-scroll-do-not', async () => {
    const p = await wordCardPage(), frozen = await openWordCard(p), dialog = p.getByRole('dialog', { name: '本地词典', exact: true });
    await dialog.locator('[data-ui-scroll="dictionary"]').dispatchEvent('scroll', { bubbles: false }); await assertFrozenWordCard(p, frozen);
    await p.evaluate(() => { const unrelated = document.createElement('div'); unrelated.dataset.testid = 'unrelated-wordcard-scroll'; document.body.append(unrelated); unrelated.dispatchEvent(new Event('scroll')); }); await assertFrozenWordCard(p, frozen);
    assert.equal(await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length), 0);
    await p.locator('[data-ui-scroll="reading"]').dispatchEvent('scroll', { bubbles: false }); await dialog.waitFor({ state: 'hidden' });
    assert.equal(await p.evaluate(id => window.uiHarness.wordCardProbe.closeRequests.includes(id), frozen.card.cardId), true);
    const second = await openWordCard(p); await p.evaluate(() => document.dispatchEvent(new Event('scroll'))); await dialog.waitFor({ state: 'hidden' });
    assert.equal(await p.evaluate(id => window.uiHarness.wordCardProbe.closeRequests.includes(id), second.card.cardId), true); await p.close();
  });
  // This generated, independently compiled probe only exposes actual React
  // mount/unmount. It does not replace WordCard's effects or native policy.
  const lifecycleInput = path.join(out, 'wordcard-lifecycle-input'); await fs.mkdir(lifecycleInput);
  const lifecycleEntry = `import React from ${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))};
import { createRoot } from ${JSON.stringify(path.join(desktop, 'node_modules/react-dom/client.js'))};
import { WordCard } from ${JSON.stringify(path.join(desktop, 'src/features/language/WordCard.tsx'))};
import { DesignPort } from ${JSON.stringify(path.join(desktop, 'tests/support/design-port.ts'))};
import { ApplicationStore } from ${JSON.stringify(path.join(desktop, 'src/bridge/projections/application-store.ts'))};
const port = new DesignPort('reading'); (${initializeWordCardTransport.toString()})(port);
const store = new ApplicationStore(port), root = createRoot(document.getElementById('owner')), anchor = document.getElementById('anchor');
let generation = 0; const callbacks = [];
const probe = { port, callbacks, mount() { const own = ++generation, unit = structuredClone(port.state.current); root.render(<WordCard key={own} unit={unit} tokenIndex={0} anchor={anchor} store={store} onClose={() => { callbacks.push(own); root.render(null); }}/>); return own; }, unmount() { root.render(null); } };
Object.assign(window, { wordCardLifecycle: probe });`;
  const lifecycleHtml = '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="anchor-owner"><button id="anchor">原句选词</button><div id="owner"></div></div><script type="module" src="./entry.tsx"></script></body></html>';
  const lifecycleConfig = `import { defineConfig } from ${JSON.stringify(path.join(desktop, 'node_modules/vite/dist/node/index.js'))}; import react from ${JSON.stringify(path.join(desktop, 'node_modules/@vitejs/plugin-react/dist/index.js'))}; export default defineConfig({ plugins:[react()],root:${JSON.stringify(lifecycleInput)},resolve:{alias:{react:${JSON.stringify(path.join(desktop, 'node_modules/react'))},'react-dom':${JSON.stringify(path.join(desktop, 'node_modules/react-dom'))}}},build:{target:'es2022',emptyOutDir:true,rollupOptions:{input:${JSON.stringify(path.join(lifecycleInput, 'index.html'))}}} });`;
  await fs.writeFile(path.join(lifecycleInput, 'entry.tsx'), lifecycleEntry); await fs.writeFile(path.join(lifecycleInput, 'index.html'), lifecycleHtml); await fs.writeFile(path.join(lifecycleInput, 'vite.config.mjs'), lifecycleConfig);
  const lifecycleBuild = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(lifecycleInput, 'vite.config.mjs'), '--outDir', path.join(out, 'wordcard-lifecycle-build')], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let lifecycleBuildLog = ''; lifecycleBuild.stdout.on('data', data => { lifecycleBuildLog += data; }); lifecycleBuild.stderr.on('data', data => { lifecycleBuildLog += data; }); const lifecycleBuildCode = await new Promise(resolve => lifecycleBuild.on('exit', resolve)); await fs.writeFile(path.join(out, 'wordcard-lifecycle-build.log'), lifecycleBuildLog); assert.equal(lifecycleBuildCode, 0, lifecycleBuildLog);
  additionalTestBuilds.push({ kind: 'real_wordcard_mount_unmount_probe', layer: 'component', nativeOrGameVerified: false, command: [process.execPath, path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(lifecycleInput, 'vite.config.mjs'), '--outDir', path.join(out, 'wordcard-lifecycle-build')], exitCode: lifecycleBuildCode, source: { entrySha256: hash(lifecycleEntry), htmlSha256: hash(lifecycleHtml), configSha256: hash(lifecycleConfig), actualWordCardSha256: hash(await fs.readFile(path.join(desktop, 'src/features/language/WordCard.tsx'))) }, buildLog: { file: 'wordcard-lifecycle-build.log', sha256: hash(lifecycleBuildLog) } });
  const lifecycleListener = createServer(); await new Promise(resolve => lifecycleListener.listen(0, '127.0.0.1', resolve)); const lifecyclePort = lifecycleListener.address().port; await new Promise(resolve => lifecycleListener.close(resolve));
  const lifecycleChild = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--config', path.join(lifecycleInput, 'vite.config.mjs'), '--outDir', path.join(out, 'wordcard-lifecycle-build'), '--host', '127.0.0.1', '--port', String(lifecyclePort), '--strictPort'], { cwd: desktop, stdio: 'ignore' }); additionalChildren.push(lifecycleChild);
  const lifecycleUrl = `http://127.0.0.1:${lifecyclePort}/`;
  for (let attempt = 0; attempt < 100; attempt++) { try { if ((await fetch(lifecycleUrl)).ok) break; } catch {} await new Promise(resolve => setTimeout(resolve, 100)); }
  const lifecyclePage = async () => { const p = await browser.newPage(); p.setDefaultTimeout(8000); p.on('pageerror', error => errors.push(error.message)); await navigateWithProductionCsp(p, lifecycleUrl); await p.waitForFunction(() => window.wordCardLifecycle); return p; };
  await check('RBC-65-owner-unmount-and-late-open-close-exact-card', async () => {
    const p = await lifecyclePage(); await p.evaluate(() => window.wordCardLifecycle.mount()); await p.waitForFunction(() => window.wordCardLifecycle.port.wordCardProbe.openReplies.length === 1);
    const first = await p.evaluate(() => window.wordCardLifecycle.port.wordCard.cardId); await p.evaluate(() => window.wordCardLifecycle.unmount()); await p.waitForFunction(() => window.wordCardLifecycle.port.wordCard === null);
    assert.deepEqual(await p.evaluate(() => window.wordCardLifecycle.port.wordCardProbe.closeRequests), [first]); assert.equal(await p.evaluate(() => window.wordCardLifecycle.port.wordCardProbe.listeners.size), 0);
    await p.evaluate(() => { const h = window.wordCardLifecycle; h.port.wordCardProbe.holds.push('old-open-reply'); h.mount(); }); await p.waitForFunction(() => window.wordCardLifecycle.port.wordCardProbe.held.has('old-open-reply'));
    const pendingId = await p.evaluate(() => window.wordCardLifecycle.port.wordCard.cardId); await p.evaluate(() => window.wordCardLifecycle.unmount()); await p.waitForFunction(() => window.wordCardLifecycle.port.wordCardProbe.listeners.size === 0);
    await p.evaluate(() => window.wordCardLifecycle.mount()); await p.waitForFunction(() => window.wordCardLifecycle.port.wordCardProbe.openReplies.length === 3 && window.wordCardLifecycle.port.wordCardProbe.listeners.size === 1);
    const next = await p.evaluate(() => structuredClone(window.wordCardLifecycle.port.wordCard)); assert.notEqual(next.cardId, pendingId);
    await p.evaluate(() => window.wordCardLifecycle.port.wordCardProbe.release('old-open-reply')); await p.waitForFunction(id => window.wordCardLifecycle.port.wordCardProbe.closeRequests.includes(id), pendingId);
    assert.deepEqual(await p.evaluate(() => window.wordCardLifecycle.port.wordCard), next); assert.deepEqual(await p.evaluate(() => window.wordCardLifecycle.callbacks), []);
    await p.evaluate(() => window.wordCardLifecycle.unmount()); await p.waitForFunction(() => window.wordCardLifecycle.port.wordCard === null); assert.equal(await p.evaluate(() => window.wordCardLifecycle.port.wordCardProbe.listeners.size), 0); await p.close();
  });
  await check('RBC-66-native-close-before-open-reply-and-frozen-new-card-isolation', async () => {
    const p = await wordCardPage(), dialog = p.getByRole('dialog', { name: '本地词典', exact: true });
    await p.evaluate(() => window.uiHarness.wordCardProbe.holds.push('closed-before-reply'));
    await p.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).first().click(); await p.waitForFunction(() => window.uiHarness.wordCardProbe.held.has('closed-before-reply'));
    const pending = await p.evaluate(() => window.uiHarness.wordCard.cardId); await p.evaluate(id => window.uiHarness.wordCardProbe.emitClosed(id), pending);
    assert.equal(await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length), 0, 'Closed-before-reply is queued, not applied to an unknown card');
    await p.evaluate(() => window.uiHarness.wordCardProbe.release('closed-before-reply')); await dialog.waitFor({ state: 'hidden' }); assert.equal(await p.evaluate(() => window.uiHarness.wordCard), null);
    const second = await openWordCard(p), request = await p.evaluate(() => structuredClone(window.uiHarness.wordCardRequests.at(-1)));
    assert.notEqual(second.card.cardId, pending); assert.deepEqual(Object.keys(request).sort(), ['anchorRect', 'textRevision', 'tokenIndex', 'unitId']); assert.equal(request.unitId, second.card.unit.unitId); assert.equal(request.textRevision, second.card.unit.textRevision); assert.equal(request.tokenIndex, second.card.tokenIndex);
    await p.evaluate(id => { const port = window.uiHarness; port.wordCardProbe.emitClosed(id); port.publish({ current: { ...port.state.current, unitId: 'future-object-same-token', textRevision: 2 }, history: { units: [...port.state.history.units], nextCursor: null } }); }, pending);
    await assertFrozenWordCard(p, second); assert.deepEqual(await p.evaluate(() => window.uiHarness.wordCardRect), request.anchorRect);
    const closes = await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length);
    await p.evaluate(() => window.uiHarness.wordCardProbe.emitClosed('late-unrelated-before-another-open')); await assertFrozenWordCard(p, second); assert.equal(await p.evaluate(() => window.uiHarness.wordCardProbe.closeRequests.length), closes);
    await p.keyboard.press('Escape'); await dialog.waitFor({ state: 'hidden' }); assert.equal(await p.evaluate(() => window.uiHarness.wordCard), null); await p.close();
  });

  // These use the actual WindowPreview React component with controlled port
  // failures. They do not claim native capture, OS permission, or game proof.
  const openCandidatePreview = async p => {
    await p.getByRole('button', { name: '连接运行中的游戏…', exact: true }).first().click();
    const dialog = p.getByRole('dialog', { name: '选择正在运行的游戏', exact: true });
    await dialog.getByRole('radio').first().check();
    return { dialog, preview: dialog.getByRole('region', { name: '运行窗口预览', exact: true }) };
  };
  const emitCandidateFrame = async p => p.evaluate(async () => {
    const port = window.uiHarness, request = port.previewRequests.at(-1);
    const projection = [...port.previewProjections.values()].find(value => value.consumerId === request.consumerId && value.state !== 'stopped');
    if (!projection?.leaseId) throw Error('Controlled candidate lease is missing');
    const canvas = document.createElement('canvas'); canvas.width = 160; canvas.height = 100;
    canvas.getContext('2d').fillRect(0, 0, 160, 100);
    const jpeg = await new Promise((resolve, reject) => canvas.toBlob(value => value ? resolve(value) : reject(Error('Controlled preview JPEG failed')), 'image/jpeg'));
    const update = { projection: { ...projection, state: 'live', capturedAtMs: '1792368001000' }, frame: { leaseId: projection.leaseId, generation: projection.generation, sequence: '1', targetIdentityHash: projection.targetIdentityHash, capturedAtMs: '1792368001000', width: 160, height: 100, contentHash: 'e'.repeat(64), encoding: 'jpeg', payload: [...new Uint8Array(await jpeg.arrayBuffer())] } };
    port.emitPreview(update); return structuredClone(update);
  });
  const installPreviewStartFailure = (port = window.uiHarness) => {
    const start = port.startWindowPreview.bind(port), capability = port.previewCapability.bind(port);
    port.previewFailure = { mode: 'unavailable', capabilityReads: 0 };
    port.previewCapability = async () => { port.previewFailure.capabilityReads += 1; return capability(); };
    port.startWindowPreview = async (request) => {
      if (port.previewFailure.mode === 'ok') return start(request);
      port.previewRequests.push(structuredClone(request));
      if (port.previewFailure.mode === 'throw') throw Error('Controlled preview start IPC rejection');
      const denied = port.previewFailure.mode === 'denied';
      const retained = denied ? { leaseId: `ui-permission-lease-${port.previewRequests.length}` } : { leaseId: null };
      const result = { leaseId: retained.leaseId, generation: String(++port.previewGeneration), consumerId: request.consumerId, state: denied ? 'permission_denied' : 'unavailable', permission: denied ? 'denied' : 'granted', targetIdentityHash: 'f'.repeat(64), attemptedAtMs: '1792368000000', capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: '1792368000000', errorCode: denied ? 'screen_capture_permission' : 'controlled_preview_unsupported', detail: denied ? 'Controlled preview permission denial' : 'Controlled unsupported preview terminal state' };
      if(result.leaseId)port.previewProjections.set(result.leaseId,structuredClone(result));return result;
    };
  };
  await check('RBC-67-candidate-typed-start-failure-is-terminal-and-explicitly-retryable', async () => {
    const p = await appPage(); await p.evaluate(installPreviewStartFailure); await p.clock.install();
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session));
    const { dialog, preview } = await openCandidatePreview(p);
    await preview.locator('[data-preview-state]').count(); await p.locator('[data-preview-state="unavailable"]').waitFor();
    assert.equal(await preview.getByText('正在开启', { exact: true }).count(), 0); assert.equal(await preview.locator('img').count(), 0);
    assert.match(await previewDiagnosticText(p, preview), /Controlled unsupported preview terminal state/); assert.match(await preview.textContent(), /尚无成功画面/);
    assert.equal(await preview.getByRole('button', { name: '暂停', exact: true }).isDisabled(), true);
    await p.clock.runFor(12500);
    assert.ok(await p.evaluate(() => window.uiHarness.previewFailure.capabilityReads >= 4)); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1);
    assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before);
    await preview.scrollIntoViewIfNeeded(); await screenshot(p, 'preview-typed-start-terminal');
    await p.evaluate(() => { window.uiHarness.previewFailure.mode = 'ok'; }); await preview.getByRole('button', { name: '刷新', exact: true }).click();
    await p.waitForFunction(() => window.uiHarness.previewRequests.length === 2); await emitCandidateFrame(p); await preview.locator('img').waitFor();
    assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await preview.locator('img').evaluate(async image => { await image.decode(); return image.naturalWidth; }), 160);
    assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); await screenshot(p, 'preview-typed-failure-explicit-recovery'); await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-68-candidate-start-rejection-never-spins-or-retries-without-intent', async () => {
    const p = await appPage(); await p.evaluate(installPreviewStartFailure); await p.evaluate(() => { window.uiHarness.previewFailure.mode = 'throw'; }); await p.clock.install();
    const { dialog, preview } = await openCandidatePreview(p); await preview.getByRole('alert').waitFor();
    assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.match(await previewDiagnosticText(p, preview), /Controlled preview start IPC rejection/);
    assert.equal(await preview.getByText('正在开启', { exact: true }).count(), 0); assert.equal(await preview.locator('img').count(), 0); assert.match(await preview.textContent(), /尚无成功画面/);
    await p.clock.runFor(12500); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1); assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable');
    assert.equal(await preview.getByRole('button', { name: '暂停', exact: true }).isDisabled(), true); assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []);
    await preview.getByRole('alert').scrollIntoViewIfNeeded(); await screenshot(p, 'preview-start-rejection-terminal'); await p.evaluate(() => { window.uiHarness.previewFailure.mode = 'ok'; }); await preview.getByRole('button', { name: '刷新', exact: true }).click();
    await p.waitForFunction(() => window.uiHarness.previewRequests.length === 2); await emitCandidateFrame(p); await preview.locator('img').waitFor(); assert.equal(await preview.getByRole('alert').count(), 0); assert.equal(await preview.getAttribute('data-preview-state'), 'live');
    await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-69-candidate-permission-edge-recovers-once-without-changing-hook', async () => {
    const p = await appPage(); await p.evaluate(installPreviewStartFailure); await p.evaluate(() => { const port = window.uiHarness; port.previewFailure.mode = 'denied'; port.previewCapabilityFixture.permission = 'denied'; }); await p.clock.install();
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session)); const { dialog, preview } = await openCandidatePreview(p);
    await p.locator('[data-preview-state="permission_denied"]').waitFor(); assert.equal(await preview.getByRole('button', { name: '授权说明…', exact: true }).count(), 1);
    await p.clock.runFor(9500); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1);
    await p.evaluate(() => { const port = window.uiHarness; port.previewFailure.mode = 'ok'; port.previewCapabilityFixture.permission = 'granted';const projection=[...port.previewProjections.values()].find(value=>value.consumerId===port.previewRequests.at(-1).consumerId);port.emitPreview({projection:{...projection,state:'starting',permission:'granted',errorCode:null},frame:null}); }); await p.clock.runFor(3500);
    assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1); await emitCandidateFrame(p); await preview.locator('img').waitFor();
    await p.clock.runFor(9500); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1); assert.equal(await preview.getAttribute('data-preview-state'), 'live');
    assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before); assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-70-session-typed-terminal-clears-frame-and-only-reattaches-rust-lease', async () => {
    const p = await appPage(); await p.clock.install(); await p.getByRole('button', { name: '当前会话', exact: true }).click();await openSessionDataset(p,'正文历史'); await p.waitForFunction(() => window.uiHarness.previewAttaches.length === 1);
    const preview = p.getByRole('region', { name: '运行窗口预览', exact: true }), oldLease = await emitSessionFrame(p); await preview.locator('img').waitFor();
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session));
    await p.evaluate(lease => { const port = window.uiHarness, previous = port.previewProjections.get(lease); port.previewProjections.delete(lease); port.previewProjections.set('controlled-session-terminal', { ...previous, leaseId: null, generation: String(++port.previewGeneration), state: 'unavailable', capturedAtMs: null, contentChangedAtMs: null, errorCode: 'controlled_session_unsupported', detail: 'Controlled Rust-owned terminal preview' }); }, oldLease);
    await p.clock.runFor(3500); await p.locator('[data-preview-state="unavailable"]').waitFor(); assert.equal(await preview.locator('img').count(), 0); assert.match(await previewDiagnosticText(p, preview), /Controlled Rust-owned terminal preview/); assert.match(await preview.textContent(), /尚无成功画面/);
    assert.equal(await preview.getByRole('button', { name: '暂停', exact: true }).isDisabled(), true); assert.ok((await p.evaluate(() => window.uiHarness.previewDetaches)).includes(oldLease));
    await preview.getByRole('button', { name: '刷新', exact: true }).click(); await p.clock.runFor(9500); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []); assert.equal(await p.evaluate(() => window.uiHarness.previewAttaches.length), 1);
    await screenshot(p, 'session-preview-typed-terminal');
    await p.evaluate(() => { const port = window.uiHarness; port.previewProjections.delete('controlled-session-terminal'); port.prepareSessionPreview(port.state.session.sessionId, 'a'.repeat(64)); }); await p.clock.runFor(3500); await p.waitForFunction(() => window.uiHarness.previewAttaches.length === 2); await emitSessionFrame(p); await preview.locator('img').waitFor();
    assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before); await p.close();
  });
  await check('RBC-71-session-ipc-rejection-is-visible-without-client-start-or-stop', async () => {
    const p = await appPage('main', 1040, 760, port => { const read = port.previewSession.bind(port); port.previewReadFailure = true; port.previewSession = async () => { if (port.previewReadFailure) throw Error('Controlled preview session IPC rejection'); return read(); }; });
    await p.clock.install(); await p.getByRole('button', { name: '当前会话', exact: true }).click();await openSessionDataset(p,'正文历史'); const preview = p.getByRole('region', { name: '运行窗口预览', exact: true }); await preview.getByRole('alert').waitFor();
    assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.match(await previewDiagnosticText(p, preview), /Controlled preview session IPC rejection/); assert.equal(await preview.getByText('正在开启', { exact: true }).count(), 0);
    await p.clock.runFor(9500); assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []);
    await p.evaluate(() => { window.uiHarness.previewReadFailure = false; }); await preview.getByRole('button', { name: '刷新', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.previewAttaches.length === 1); await emitSessionFrame(p); await preview.locator('img').waitFor();
    assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await preview.getByRole('alert').count(), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); await p.close();
  });
  await check('RBC-72-candidate-transport-auto-retry-is-bounded-and-explicitly-reset', async () => {
    const p = await appPage(); await p.clock.install(); const { dialog, preview } = await openCandidatePreview(p); await p.waitForFunction(() => window.uiHarness.previewRequests.length === 1); await emitCandidateFrame(p); await preview.locator('img').waitFor();
    const fail = async () => p.evaluate(() => { const port = window.uiHarness, request = port.previewRequests.at(-1), projection = [...port.previewProjections.values()].find(value => value.consumerId === request.consumerId && value.state !== 'stopped'); port.emitPreview({ projection: { ...projection, state: 'unavailable', errorCode: 'preview_transport', detail: 'Controlled repeated transport failure' }, frame: null }); });
    await fail(); await p.clock.runFor(3500); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1);
    await fail(); await p.clock.runFor(12500); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1); assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.equal(await preview.locator('img').count(), 0); assert.match(await preview.textContent(), /尚无成功画面/);
    assert.equal(await p.evaluate(() => window.uiHarness.previewControls.filter(value => value.action === 'stop').length), 0);
    await preview.getByRole('button', { name: '刷新', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.previewControls.some(value=>value.action==='resume'));assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),1); await emitCandidateFrame(p); await preview.locator('img').waitFor(); assert.equal(await preview.getAttribute('data-preview-state'), 'live');
    assert.equal(await p.evaluate(() => window.uiHarness.previewControls.filter(value => value.action === 'stop').length), 0); await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-73-candidate-target-change-and-cancel-isolate-late-start', async () => {
    const p = await appPage('main', 1040, 760, port => { const start = port.startWindowPreview.bind(port); port.targetCandidates.push({ ...port.targetCandidates[0], candidateId: 'other-preview-target', windowTitle: 'Other controlled preview window', windowsPid: 654 }); port.startWindowPreview = async (request) => { const result = await start(request); if (request.candidateId === 'ui-target') { port.heldPreviewStart = { result }; await new Promise(resolve => { port.releaseHeldPreviewStart = resolve; }); } return result; }; });
    const { dialog, preview } = await openCandidatePreview(p); await p.waitForFunction(() => !!window.uiHarness.heldPreviewStart); const oldLease = await p.evaluate(() => window.uiHarness.heldPreviewStart.result.leaseId);
    await dialog.getByRole('radio').nth(1).check(); await p.waitForFunction(() => window.uiHarness.previewRequests.length === 2); const current = await emitCandidateFrame(p); await preview.locator('img').waitFor(); const image = await preview.locator('img').getAttribute('src');
    await p.evaluate(() => { const port = window.uiHarness; port.releaseHeldPreviewStart(); const previous = port.heldPreviewStart; port.emitPreview({ projection: { ...previous.result, state: 'unavailable', errorCode: 'controlled_late_old_target', detail: 'Late old-target terminal state' }, frame: null }); });
    await p.waitForFunction(lease => window.uiHarness.previewControls.some(value => value.leaseId === lease && value.action === 'stop'), oldLease);
    assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await preview.locator('img').getAttribute('src'), image); assert.equal(await preview.getByText('Late old-target terminal state', { exact: true }).count(), 0);
    assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.at(-1).candidateId), 'other-preview-target'); assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.waitForFunction(lease => window.uiHarness.previewControls.some(value => value.leaseId === lease && value.action === 'stop'), current.projection.leaseId);
    assert.equal(await p.getByRole('dialog').count(), 0); assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); await p.close();
  });


  const installCandidatePickerRequired = (port = window.uiHarness) => {
    const start = port.startWindowPreview.bind(port);
    port.startWindowPreview = async (request) => {
      const previous = await start(request);
      const result = { ...previous, state: 'unavailable', errorCode: 'preview_window_selection_required', detail: 'Controlled system single-window selection is required', capturedAtMs: null, contentChangedAtMs: null };
      port.previewProjections.set(result.leaseId, result); return structuredClone(result);
    };
  };
  const queuePickerReply = async (p, outcome = 'selected', holdKey = null, session = false) => p.evaluate(({ outcome, holdKey, session }) => {
    const port = window.uiHarness, consumerId = session ? port.state.session.sessionId : port.previewRequests.at(-1).consumerId;
    const previous = [...port.previewProjections.values()].find(value => value.consumerId === consumerId && value.state !== 'stopped');
    if (!previous) throw Error('Explicit picker control slot is missing');
    const generation = String(++port.previewGeneration), selected = outcome === 'selected';
    const projection = { ...previous, consumerId: undefined, leaseId: selected ? `ui-picker-lease-${generation}` : previous.leaseId, generation, state: selected ? 'starting' : 'unavailable', errorCode: selected ? null : `preview_selection_${outcome}`, detail: selected ? 'Controlled explicitly paired window' : `Controlled system selection ${outcome}`, capturedAtMs: null, contentChangedAtMs: null };
    delete projection.consumerId;
    port.previewPickerFixtures.push({ projection, ...(holdKey ? { holdKey } : {}) }); return structuredClone(projection);
  }, { outcome, holdKey, session });
  const makeSessionPickerRequired = port => {
    const previous = [...port.previewProjections.values()].find(value => value.consumerId === port.state.session.sessionId && value.state !== 'stopped');
    Object.assign(previous, { state: 'unavailable', errorCode: 'preview_window_selection_required', detail: 'Controlled Rust session needs explicit system window selection', capturedAtMs: null, contentChangedAtMs: null });
  };
  await check('RBC-74-picker-requires-explicit-candidate-intent-and-shows-selected-frame', async () => {
    const p = await appPage(); await p.evaluate(installCandidatePickerRequired); await p.clock.install(); const { dialog, preview } = await openCandidatePreview(p);
    const chooser = preview.getByRole('button', { name: '重新选择…', exact: true }); await chooser.waitFor();
    assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.equal(await preview.getByRole('button', { name: '暂停', exact: true }).isDisabled(), true); assert.equal(await preview.locator('img').count(), 0);
    await p.clock.runFor(9500); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.length), 0); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1);
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session)); await preview.getByRole('button', { name: '刷新', exact: true }).click();
    await p.waitForFunction(() => window.uiHarness.previewControls.some(value => value.action === 'resume')); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.length), 0); assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable');
    const slot = await p.evaluate(() => [...window.uiHarness.previewProjections.values()].find(value => value.consumerId === window.uiHarness.previewRequests.at(-1).consumerId && value.state !== 'stopped'));
    await chooser.scrollIntoViewIfNeeded(); await screenshot(p, 'preview-explicit-system-picker-required'); await queuePickerReply(p); await chooser.click(); await p.waitForFunction(() => window.uiHarness.previewPickerRequests.length === 1);
    const request = await p.evaluate(() => window.uiHarness.previewPickerRequests[0]); assert.equal(request.candidateId, 'ui-target'); assert.equal(request.consumer, 'candidate'); assert.equal(request.consumerId, slot.consumerId); assert.equal(request.expectedGeneration, slot.generation);
    await emitCandidateFrame(p); await preview.locator('img').waitFor(); assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await preview.locator('img').evaluate(async image => { await image.decode(); return image.naturalWidth; }), 160);
    assert.equal(await chooser.count(), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before); assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); await screenshot(p, 'preview-explicit-system-picker-selected'); await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-75-picker-cancel-timeout-mismatch-busy-and-ipc-failure-stay-terminal', async () => {
    const p = await appPage(); await p.evaluate(installCandidatePickerRequired); await p.clock.install(); const { dialog, preview } = await openCandidatePreview(p);
    const chooser = preview.getByRole('button', { name: '重新选择…', exact: true }); await chooser.waitFor(); const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session));
    for (const outcome of ['cancelled', 'timeout', 'mismatch', 'busy', 'expired']) {
      await queuePickerReply(p, outcome); const beforeCount = await p.evaluate(() => window.uiHarness.previewPickerRequests.length); await chooser.click(); await p.waitForFunction(count => window.uiHarness.previewPickerRequests.length === count + 1, beforeCount);
      assert.ok((await previewDiagnosticText(p, preview)).includes(`Controlled system selection ${outcome}`)); assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.equal(await chooser.isEnabled(), true); assert.equal(await preview.getByRole('button', { name: '暂停', exact: true }).isDisabled(), true); assert.equal(await preview.locator('img').count(), 0); assert.match(await preview.textContent(), /尚无成功画面/);
      await p.clock.runFor(6500); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.length), beforeCount + 1); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 1); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before);
    }
    await chooser.scrollIntoViewIfNeeded(); await screenshot(p, 'preview-system-picker-cancel-and-failure');
    await p.evaluate(() => window.uiHarness.previewPickerFixtures.push({ error: 'Controlled system picker IPC rejection' })); await chooser.click(); await preview.getByRole('alert').waitFor(); assert.equal(await preview.getAttribute('data-preview-state'), 'unavailable'); assert.match(await previewDiagnosticText(p, preview), /Controlled system picker IPC rejection/); assert.equal(await chooser.isEnabled(), true);
    await queuePickerReply(p, 'unsupported'); await chooser.click(); assert.match(await previewDiagnosticText(p, preview), /Controlled system selection unsupported/); assert.equal(await chooser.isDisabled(), true); assert.equal(await preview.locator('img').count(), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before);
    await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-76-picker-late-candidate-target-result-cannot-replace-next-preview', async () => {
    const p = await appPage('main', 1040, 760, port => { port.targetCandidates.push({ ...port.targetCandidates[0], candidateId: 'next-picker-target', windowTitle: 'Next controlled picker target', windowsPid: 654 }); }); await p.evaluate(installCandidatePickerRequired);
    const { dialog, preview } = await openCandidatePreview(p); const chooser = preview.getByRole('button', { name: '重新选择…', exact: true }); await chooser.waitFor(); const old = await queuePickerReply(p, 'selected', 'late-target-picker'); await chooser.click(); await p.waitForFunction(() => window.uiHarness.previewPickerPending.has('late-target-picker'));
    await dialog.getByRole('radio').nth(1).check(); await p.waitForFunction(() => window.uiHarness.previewRequests.length === 2); await chooser.waitFor(); await queuePickerReply(p); await chooser.click(); await p.waitForFunction(() => window.uiHarness.previewPickerRequests.length === 2); const current = await emitCandidateFrame(p); await preview.locator('img').waitFor(); const image = await preview.locator('img').getAttribute('src');
    await p.evaluate(() => window.uiHarness.releasePreviewPicker('late-target-picker')); await p.waitForFunction(lease => window.uiHarness.previewControls.some(value => value.action === 'stop' && value.leaseId === lease), old.leaseId);
    assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await preview.locator('img').getAttribute('src'), image); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.at(-1).candidateId), 'next-picker-target'); assert.equal(await p.evaluate(lease => window.uiHarness.previewProjections.get(lease).state, current.projection.leaseId), 'live');
    assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); await dialog.getByRole('button', { name: '取消', exact: true }).click(); await p.close();
  });
  await check('RBC-77-session-picker-is-explicit-and-does-not-client-start-or-stop', async () => {
    const p = await appPage('main', 1040, 760, makeSessionPickerRequired); await p.clock.install(); await p.getByRole('button', { name: '当前会话', exact: true }).click();await openSessionDataset(p,'正文历史'); const preview = p.getByRole('region', { name: '运行窗口预览', exact: true }), chooser = preview.getByRole('button', { name: '重新选择…', exact: true }); await chooser.waitFor();
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session)); await p.clock.runFor(9500); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.length), 0); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.previewControls), []);
    await preview.getByRole('button', { name: '刷新', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.previewControls.length === 1); assert.equal(await p.evaluate(() => window.uiHarness.previewControls[0].action), 'resume'); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.length), 0);
    const slot = await p.evaluate(() => [...window.uiHarness.previewProjections.values()].find(value => value.consumerId === window.uiHarness.state.session.sessionId && value.state !== 'stopped')); await queuePickerReply(p, 'selected', null, true); await chooser.click(); await p.waitForFunction(() => window.uiHarness.previewPickerRequests.length === 1);
    const request = await p.evaluate(() => window.uiHarness.previewPickerRequests[0]); assert.equal(request.candidateId, ''); assert.equal(request.consumer, 'session'); assert.equal(request.consumerId, before.sessionId); assert.equal(request.expectedGeneration, slot.generation);
    const lease = await emitSessionFrame(p); await preview.locator('img').waitFor(); assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); assert.equal(await p.evaluate(() => window.uiHarness.previewControls.filter(value => value.action === 'stop').length), 0);
    await p.getByRole('button', { name: '游戏库', exact: true }).click(); assert.equal(await p.evaluate(lease => window.uiHarness.previewProjections.get(lease).state, lease), 'live'); assert.equal(await p.evaluate(() => window.uiHarness.previewControls.filter(value => value.action === 'stop').length), 0); await p.close();
  });
  await check('RBC-78-picker-late-session-result-only-detaches-old-session', async () => {
    const p = await appPage('main', 1040, 760, makeSessionPickerRequired); await p.getByRole('button', { name: '当前会话', exact: true }).click();await openSessionDataset(p,'正文历史'); const preview = p.getByRole('region', { name: '运行窗口预览', exact: true }); await preview.getByRole('button', { name: '重新选择…', exact: true }).waitFor();
    const attachedBeforeSelection=await p.evaluate(()=>window.uiHarness.previewAttaches.at(-1));
    const previous = await queuePickerReply(p, 'selected', 'late-session-picker', true); await preview.getByRole('button', { name: '重新选择…', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.previewPickerPending.has('late-session-picker'));
    await p.evaluate(() => { const port = window.uiHarness; port.bindingFixture = { ...port.bindingFixture, sessionId: 'next-picker-session' }; port.prepareSessionPreview('next-picker-session', 'b'.repeat(64)); port.publish({ session: { ...port.state.session, sessionId: 'next-picker-session' } }); }); await p.waitForFunction(() => window.uiHarness.previewSessionQueries.includes('next-picker-session'));
    const nextLease = await emitSessionFrame(p); await preview.locator('img').waitFor(); const image = await preview.locator('img').getAttribute('src'); await p.evaluate(() => window.uiHarness.releasePreviewPicker('late-session-picker'));
    await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.ok((await p.evaluate(()=>window.uiHarness.previewDetaches)).includes(attachedBeforeSelection),'Exact old subscription was released at unmount');assert.equal((await p.evaluate(()=>window.uiHarness.previewAttaches)).includes(previous.leaseId),false,'Late control result never installed a hidden listener'); assert.equal(await preview.getAttribute('data-preview-state'), 'live'); assert.equal(await preview.locator('img').getAttribute('src'), image); assert.equal(await p.evaluate(lease => window.uiHarness.previewProjections.get(lease).state, nextLease), 'live'); assert.equal(await p.evaluate(() => window.uiHarness.previewRequests.length), 0); assert.equal(await p.evaluate(() => window.uiHarness.previewControls.filter(value => value.action === 'stop').length), 0); assert.equal(await p.evaluate(() => window.uiHarness.previewPickerRequests.length), 1); await p.close();
  });
  await check('RBC-79-picker-candidate-cancel-releases-late-selected-lease', async () => {
    const p = await appPage(); await p.evaluate(installCandidatePickerRequired); const { dialog, preview } = await openCandidatePreview(p); await preview.getByRole('button', { name: '重新选择…', exact: true }).waitFor(); const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session));
    const previous = await queuePickerReply(p, 'selected', 'cancelled-view-picker'); await preview.getByRole('button', { name: '重新选择…', exact: true }).click(); await p.waitForFunction(() => window.uiHarness.previewPickerPending.has('cancelled-view-picker')); await dialog.getByRole('button', { name: '取消', exact: true }).click(); assert.equal(await p.getByRole('dialog').count(), 0);
    await p.evaluate(() => window.uiHarness.releasePreviewPicker('cancelled-view-picker')); await p.waitForFunction(lease => window.uiHarness.previewControls.some(value => value.action === 'stop' && value.leaseId === lease), previous.leaseId); assert.equal(await p.evaluate(lease => window.uiHarness.previewProjections.get(lease).state, previous.leaseId), 'stopped'); assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before); await p.close();
  });
  await check('RBC-80-real-tauri-picker-adapter-uses-only-explicit-narrow-select-ipc', async () => {
    const calls = [], received = [], projections = new Map();
    const invoke = async (name, args) => { calls.push({ name, args }); if (name === 'application_preview_select_window') { const projection = { leaseId: `controlled-picker-${args.request.consumer}`, generation: args.request.expectedGeneration, consumerId: args.request.consumerId, state: 'starting', permission: 'granted', targetIdentityHash: 'f'.repeat(64), attemptedAtMs: null, capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: null, errorCode: null, detail: 'Controlled picker adapter result' }; projections.set(projection.leaseId, projection); return projection; } if (name === 'application_preview_latest') return { projection: projections.get(args.leaseId), frame: null }; throw Error(`Unexpected picker native call ${name}`); };
    const stubs = { '@tauri-apps/api/event': { listen: async () => () => {} }, '@tauri-apps/api/core': { Channel: class {}, invoke }, '@tauri-apps/api/window': { getCurrentWindow: () => ({}) }, '@tauri-apps/api/webview': { getCurrentWebview: () => ({}) } }; const adapter = loadActualTauriPort(stubs);
    assert.deepEqual(calls, []); for (const consumer of ['candidate', 'session']) { const request = { candidateId: consumer === 'session' ? '' : 'controlled-picker-target', consumer, consumerId: `${consumer}-picker-owner`, expectedGeneration: '7' }; const result = await adapter.selectWindowPreview(request); assert.equal(calls.filter(value=>value.name==='application_preview_latest').length,received.length);const unsubscribe=adapter.attachWindowPreview(result,value=>received.push(value)); await new Promise(resolve => setTimeout(resolve, 20)); assert.deepEqual(calls.find(value => value.name === 'application_preview_select_window' && value.args.request.consumer === consumer).args.request, request); assert.equal(received.at(-1).projection.leaseId, result.leaseId); unsubscribe(); }
    assert.equal(calls.filter(value => value.name === 'application_preview_select_window').length, 2); assert.equal(calls.filter(value => value.name === 'application_preview_latest').length, 2); assert.equal(calls.filter(value => value.name === 'application_preview_start' || value.name === 'application_preview_control').length, 0);
  });


  await check('RBC-81-real-tauri-late-select-or-start-reply-cannot-replace-new-pull', async () => {
    for (const oldKind of ['select', 'start']) {
      const calls = [], oldReceived = [], currentReceived = [], pending = new Map(), projections = new Map(), cancellations=[]; let resolveOld;
      const projection = (suffix, request) => ({ leaseId: `controlled-race-${oldKind}-${suffix}`, generation: suffix === 'old' ? '7' : '9', consumerId: request.consumerId, state: 'starting', permission: 'granted', targetIdentityHash: (suffix === 'old' ? 'a' : 'b').repeat(64), attemptedAtMs: null, capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: null, errorCode: null, detail: `Controlled ${suffix} transport` });
      const invoke = async (name, args) => {
        calls.push({ name, args });
        if (name === 'application_preview_select_window' || name === 'application_preview_start') {
          const old = args.request.candidateId === 'old-candidate', result = projection(old ? 'old' : 'new', args.request); projections.set(result.leaseId, result);
          if (old) { pending.set('old-result', result); await new Promise(resolve => { resolveOld = resolve; }); } return result;
        }
        if (name === 'application_preview_latest') return { projection: projections.get(args.leaseId), frame: null };
        if (name === 'application_preview_control') return { ...projections.get(args.request.leaseId), state: 'stopped' };
        throw Error(`Unexpected out-of-order preview call ${name}`);
      };
      const stubs = { '@tauri-apps/api/event': { listen: async () => () => {} }, '@tauri-apps/api/core': { Channel: class {}, invoke }, '@tauri-apps/api/window': { getCurrentWindow: () => ({}) }, '@tauri-apps/api/webview': { getCurrentWebview: () => ({}) } }; const adapter = loadActualTauriPort(stubs);
      try {
      const request = { candidateId: 'old-candidate', consumer: 'candidate', consumerId: `same-consumer-${oldKind}`, expectedGeneration: '6' };
      const oldPromise = (oldKind === 'select' ? adapter.selectWindowPreview(request) : adapter.startWindowPreview(request));
      assert.equal(pending.has('old-result'), true);
      const nextRequest = { ...request, candidateId: 'new-candidate', expectedGeneration: '8' };
      const current = await (oldKind === 'select' ? adapter.startWindowPreview(nextRequest) : adapter.selectWindowPreview(nextRequest));
      const unsubscribeCurrent=adapter.attachWindowPreview(current,value=>currentReceived.push(value));cancellations.push(unsubscribeCurrent);await new Promise(resolve => setTimeout(resolve, 20)); assert.equal(currentReceived.length, 1); resolveOld(); const old = await oldPromise; await new Promise(resolve => setTimeout(resolve, 20));
      assert.equal(oldReceived.length, 0, 'The old reply must not install a stale pull before React can reject its epoch'); assert.equal(calls.filter(value => value.name === 'application_preview_latest' && value.args.leaseId === old.leaseId).length, 0);
      await adapter.controlWindowPreview({ leaseId: old.leaseId, generation: old.generation, action: 'stop' }); assert.equal(oldReceived.length,0);
      await new Promise(resolve => setTimeout(resolve, 550)); assert.ok(currentReceived.length >= 2, 'Exact stale-candidate cleanup must leave the new lease polling'); assert.ok(calls.filter(value => value.name === 'application_preview_latest' && value.args.leaseId === current.leaseId).length >= 2);
      assert.equal(calls.filter(value => value.name === 'application_preview_control' && value.args.request.leaseId === current.leaseId).length, 0); assert.equal(calls.filter(value => value.name === 'application_preview_start' || value.name === 'application_preview_select_window').length, 2);
      } finally { for (const unsubscribe of cancellations) unsubscribe(); }
    }
  });

  const settleReact = p => p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const typedBindingBusy = {code:'capture_cleanup_pending',stage:'capture',message:'Controlled typed binding is draining',recoverable:true};
  async function waitUntil(predicate, timeout=4000) { const end=Date.now()+timeout;while(!predicate()){if(Date.now()>end)throw Error('Controlled adapter observation deadline expired');await new Promise(resolve=>setTimeout(resolve,10));} }
  async function actualAdapter(invoke) {
    const stubs={'@tauri-apps/api/event':{listen:async()=>()=>{}},'@tauri-apps/api/core':{Channel:class{},invoke},'@tauri-apps/api/window':{getCurrentWindow:()=>({})},'@tauri-apps/api/webview':{getCurrentWebview:()=>({})}};
    return loadActualTauriPort(stubs);
  }
  async function adapterFixture() {
    const fixture={calls:[],sequence:0,inFlight:0,maxInFlight:0,replies:[],projection:{leaseId:'controlled-multicast-lease',generation:'1',consumerId:'controlled-shared-session',state:'live',permission:'granted',targetIdentityHash:'a'.repeat(64),attemptedAtMs:'1792368000000',capturedAtMs:'1792368001000',contentChangedAtMs:'1792368001000',permissionCheckedAtMs:'1792368000000',errorCode:null,detail:'Explicit controlled transport projection'},cancellations:[]};
    fixture.update=()=>{const sequence=String(++fixture.sequence);return {projection:{...fixture.projection},frame:{leaseId:fixture.projection.leaseId,generation:fixture.projection.generation,sequence,targetIdentityHash:fixture.projection.targetIdentityHash,capturedAtMs:String(1792368001000+fixture.sequence),width:160,height:100,contentHash:'b'.repeat(64),encoding:'jpeg',payload:[1,2,3]}};};
    fixture.adapter=await actualAdapter(async(name,args)=>{fixture.calls.push({name,args});if(name!=='application_preview_latest')throw Error(`Subscription unexpectedly invoked ${name}`);fixture.inFlight++;fixture.maxInFlight=Math.max(fixture.maxInFlight,fixture.inFlight);try{return fixture.replies.length?await fixture.replies.shift()(args):fixture.update();}finally{fixture.inFlight--;}});
    fixture.attach=listener=>{const off=fixture.adapter.attachWindowPreview({...fixture.projection},listener);fixture.cancellations.push(off);return off;};fixture.close=()=>fixture.cancellations.forEach(off=>off());return fixture;
  }
  const nativePreviewStub = () => {
    window.previewNativeCalls=[];window.previewNativeFailure=null;window.previewNativeAutoSequence=false;
    window.__TAURI_INTERNALS__={invoke:async(name,args={})=>{
      const port=window.uiHarness;window.previewNativeCalls.push({name,args});
      if(name==='application_preview_session')return port.previewSession();
      if(name==='application_preview_capability')return port.previewCapability();
      if(name==='application_preview_select_window')return port.selectWindowPreview(args.request);
      if(name==='application_preview_control')return port.controlWindowPreview(args.request);
      if(name==='application_game_window_binding')return port.gameWindowBinding();
      if(name==='application_session_records')return port.sessionRecords(args.query);
      if(name==='application_preview_latest'){
        if(window.previewNativeFailure)throw structuredClone(window.previewNativeFailure);
        const projection=structuredClone(port.previewProjections.get(args.leaseId));if(!projection)throw {code:'preview_target_closed',stage:'capture',message:'Controlled closed target',recoverable:false};
        let frame=structuredClone(port.previewFrameFixtures.get(`${args.leaseId}:${args.generation}`)??null);
        if(frame&&window.previewNativeAutoSequence){frame.sequence=String(++window.previewNativeSequence);frame.capturedAtMs=String(1792368001000+window.previewNativeSequence);projection.capturedAtMs=frame.capturedAtMs;}
        if(frame&&args.afterSequence!==null&&BigInt(frame.sequence)<=BigInt(args.afterSequence))frame=null;
        if(frame)window.previewNativeLastFrame=structuredClone(frame);
        return {projection,frame};
      }
      throw Error(`Unexpected controlled native preview call ${name}`);
    }};
  };
  const previewProbePage = async (mode, initialize) => {
    const p=await browser.newPage({viewport:{width:1040,height:900},deviceScaleFactor:1});p.setDefaultTimeout(16000);p.on('pageerror',error=>errors.push(error.message));if(initialize)await p.addInitScript({content:`window.__initializeControlledPort=${initialize.toString()}`});await p.addInitScript(nativePreviewStub);
    await navigateWithProductionCsp(p,`http://127.0.0.1:${port}/tests/visual/index.html?scenario=reading&probe=reading-controls&subscriptionProbe=${mode}`);await p.waitForFunction(()=>window.uiHarness&&document.querySelector('[aria-label="运行窗口预览"]'));return p;
  };
  const establishedReading = async () => {const p=await appPage();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===1);await emitSessionFrame(p);const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await preview.locator('img').waitFor();assert.equal(await preview.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);return {p,preview};};
  const assertStaleCacheSubscription = async joiningId => {
    const p=await previewProbePage('adapter'),retainedId=joiningId==='A'?'B':'A';try{
      await p.getByRole('button',{name:`切换订阅 ${joiningId}`,exact:true}).click();const retained=p.getByTestId(`preview-subscription-${retainedId}`),joining=p.getByTestId(`preview-subscription-${joiningId}`);assert.equal(await joining.count(),0);await emitSessionFrame(p);await retained.locator('img').waitFor();const retainedImage=await retained.locator('img').getAttribute('src'),retainedTime=await previewTimeText(retained);
      await p.evaluate(()=>window.previewNativeFailure={code:'preview_transport',stage:'capture',message:'Controlled failed shared read before cache replay',recoverable:true});await retained.locator('[data-preview-state="stale"]').waitFor();
      // Only the transport is held. The real production adapter owns cache
      // replay, single-flight, listener tokens and delivery to both components.
      await p.evaluate(()=>{const invoke=window.__TAURI_INTERNALS__.invoke;window.previewNativeReadBlocks=[];window.previewNativeBlockReads=true;window.previewNativeFailure=null;window.__TAURI_INTERNALS__.invoke=async(name,args)=>{if(name==='application_preview_latest'&&window.previewNativeBlockReads)await new Promise(resolve=>window.previewNativeReadBlocks.push(resolve));return invoke(name,args);};});await p.waitForFunction(()=>window.previewNativeReadBlocks.length===1);
      await p.getByRole('button',{name:`切换订阅 ${joiningId}`,exact:true}).click();await joining.locator('img').waitFor();await joining.locator('[data-preview-state="stale"]').waitFor();const joinedImage=await joining.locator('img').getAttribute('src'),metadata=await p.evaluate(()=>window.uiHarness.previewSessionQueries.length);
      await p.waitForFunction(count=>window.uiHarness.previewSessionQueries.length>=count+2,metadata);await settleReact(p);
      assert.equal(await retained.getByRole('region',{name:'运行窗口预览',exact:true}).getAttribute('data-preview-state'),'stale');assert.equal(await joining.getByRole('region',{name:'运行窗口预览',exact:true}).getAttribute('data-preview-state'),'stale','A cached stale JPEG is not a recovered Live frame after metadata');assert.match(await joining.locator('img').getAttribute('alt'),/非实时/);assert.equal(await retained.locator('img').getAttribute('src'),retainedImage);assert.equal(await joining.locator('img').getAttribute('src'),joinedImage);assert.equal(await previewTimeText(joining),retainedTime);assert.equal(await p.evaluate(()=>window.previewNativeReadBlocks.length),1,'A new listener cannot create a second upstream pending read');
      await p.evaluate(()=>{const port=window.uiHarness,update=structuredClone(port.savedPreviewUpdate);update.frame.sequence='2';update.frame.capturedAtMs='1792368002000';update.projection.capturedAtMs='1792368002000';port.emitPreview(update);window.previewNativeBlockReads=false;window.previewNativeReadBlocks.splice(0).forEach(resolve=>resolve());});await retained.locator('[data-preview-state="live"]').waitFor();await joining.locator('[data-preview-state="live"]').waitFor();assert.notEqual(await retained.locator('img').getAttribute('src'),retainedImage);assert.notEqual(await joining.locator('img').getAttribute('src'),joinedImage);assert.equal(await joining.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);const freshTime=new Date(1792368002000).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3});assert.equal(await previewTimeText(joining),`画面时间 ${freshTime}`);assert.equal(await p.evaluate(()=>window.previewNativeCalls.some(call=>['application_preview_start','application_preview_select_window','application_preview_control'].includes(call.name))),false);
    }finally{await p.close();}
  };
  const assertCapabilityOnlyDenial = async mode => {
    const p=await previewProbePage(mode);try{
      await emitSessionFrame(p);const preview=p.getByRole('region',{name:'运行窗口预览',exact:true}).first();await preview.locator('img').waitFor();const initial=await p.evaluate(()=>({session:structuredClone(window.uiHarness.state.session),current:structuredClone(window.uiHarness.state.current)}));
      await p.evaluate(()=>{const invoke=window.__TAURI_INTERNALS__.invoke,port=window.uiHarness;window.capabilityReadReceipts=[];const capability=port.previewCapability.bind(port);port.previewCapability=async()=>{const value=await capability();window.capabilityReadReceipts.push(value.permission);return value;};window.permissionOldReads=[];window.holdPermissionOldRead=true;window.__TAURI_INTERNALS__.invoke=async(name,args)=>{if(name==='application_preview_latest'&&window.holdPermissionOldRead){const old=structuredClone(port.savedPreviewUpdate);old.frame.sequence='2';old.frame.capturedAtMs='1792368002000';old.projection.capturedAtMs='1792368002000';await new Promise(resolve=>window.permissionOldReads.push(resolve));return old;}return invoke(name,args);};});
      await p.waitForFunction(()=>window.permissionOldReads.length===1);await p.evaluate(()=>{const port=window.uiHarness;port.previewCapabilityFixture={...port.previewCapabilityFixture,permission:'denied',errorCode:'preview_permission_denied',detail:'Controlled capability-only denial; old projection intentionally remains Live'};});await p.locator('[data-preview-state="permission_revoked"]').first().waitFor();assert.equal(await p.getByRole('region',{name:'运行窗口预览',exact:true}).locator('img').count(),0);assert.match(await preview.locator('time').textContent(),/尚无成功画面/);
      await p.evaluate(()=>{window.holdPermissionOldRead=false;window.permissionOldReads.splice(0).forEach(resolve=>resolve());});await settleReact(p);await p.waitForTimeout(600);assert.equal(await preview.getAttribute('data-preview-state'),'permission_revoked');assert.equal(await p.getByRole('region',{name:'运行窗口预览',exact:true}).locator('img').count(),0,'A higher-sequence same-generation old Live reply cannot redraw after capability denied');assert.match(await preview.locator('time').textContent(),/尚无成功画面/);
      const readsBeforeGrant=await p.evaluate(()=>window.previewNativeCalls.filter(call=>call.name==='application_preview_latest').length);await p.evaluate(()=>{const port=window.uiHarness;port.previewCapabilityFixture={...port.previewCapabilityFixture,permission:'granted',errorCode:null,detail:'Controlled restored permission receipt'};});await p.waitForFunction(()=>window.capabilityReadReceipts.at(-1)==='granted');await p.waitForFunction(count=>window.previewNativeCalls.filter(call=>call.name==='application_preview_latest').length>count,readsBeforeGrant);await settleReact(p);assert.equal(await preview.locator('img').count(),0,'Grant plus an already displayed old sequence is not a new successful frame');assert.match(await preview.locator('time').textContent(),/尚无成功画面/);
      await p.evaluate(()=>{const port=window.uiHarness,update=structuredClone(port.savedPreviewUpdate);update.frame.sequence='3';update.frame.capturedAtMs='1792368003000';update.projection.capturedAtMs='1792368003000';port.emitPreview(update);});await p.locator('[data-preview-state="live"]').first().waitFor();assert.equal(await preview.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);const expected=new Date(1792368003000).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3});assert.equal(await previewTimeText(preview),`画面时间 ${expected}`);assert.deepEqual(await p.evaluate(()=>({session:window.uiHarness.state.session,current:window.uiHarness.state.current})),initial);assert.equal(await p.evaluate(()=>window.previewNativeCalls.some(call=>['application_preview_start','application_preview_select_window','application_preview_control'].includes(call.name))),false);
    }finally{await p.close();}
  };
  const assertInitialBoundPreviewEntry = async () => {
    const p=await previewProbePage('reading-adapter',port=>{port.bindingFixture={...port.bindingFixture,verifiedAtMs:null};});try{
      const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await p.locator('[data-preview-state="starting"]').first().waitFor();assert.equal(await preview.locator('img').count(),0);assert.match(await preview.textContent(),/尚无成功画面/);assert.equal(await p.evaluate(()=>window.uiHarness.bindingFixture.verifiedAtMs),null);
      await p.evaluate(()=>{const port=window.uiHarness,projection=structuredClone([...port.previewProjections.values()].at(-1));projection.state='unavailable';projection.errorCode='preview_window_selection_required';projection.detail='Controlled already-associated ANSI window requires explicit native picker';port.emitPreview({projection,frame:null});port.previewPickerFixtures.push({projection:{...projection,generation:String(BigInt(projection.generation)+1n),state:'starting',errorCode:null,detail:'Controlled explicit native Selecting transaction; no frame yet'}});});
      const choose=preview.getByRole('button',{name:'重新选择…',exact:true});await choose.waitFor();assert.equal(await choose.isEnabled(),true);assert.equal(await p.getByText('尚无可核验的关联窗口',{exact:true}).count(),0);await choose.click();await p.locator('[data-preview-state="starting"]').first().waitFor();assert.equal(await p.evaluate(()=>window.uiHarness.previewPickerRequests.length),1);assert.equal(await p.evaluate(()=>window.uiHarness.previewPickerRequests[0].consumer),'session');assert.equal(await preview.locator('img').count(),0);assert.equal(await p.evaluate(()=>window.uiHarness.bindingFixture.verifiedAtMs),null);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);
    }finally{await p.close();}
    const closed=await previewProbePage('reading-adapter',port=>{port.bindingFixture={...port.bindingFixture,state:'stale',verifiedAtMs:null,errorCode:'preview_target_closed',detail:'Controlled truly closed associated window'};});try{await closed.getByText('关联窗口已失效',{exact:true}).waitFor();assert.equal(await closed.getByRole('button',{name:'重新选择…',exact:true}).count(),0);assert.equal(await closed.getByRole('region',{name:'运行窗口预览',exact:true}).locator('img').count(),0);assert.equal(await closed.evaluate(()=>window.uiHarness.previewSessionQueries.length),0);}finally{await closed.close();}
  };
  await check('RBC-82-typed-transient-binding-rejection-retains-decoded-frame-and-subscription',async()=>{
    const {p,preview}=await establishedReading();try{
      const before=await p.evaluate(()=>{const node=document.querySelector('[aria-label="运行窗口预览"]');window.retainedPreviewNode=node;return {url:node.querySelector('img').src,detaches:window.uiHarness.previewDetaches.length,session:structuredClone(window.uiHarness.state.session),projection:structuredClone([...window.uiHarness.previewProjections.values()].at(-1))};});
      await p.evaluate(error=>window.uiHarness.bindingResponses.push({error}),typedBindingBusy);await p.waitForFunction(()=>window.uiHarness.bindingRejected.length===1);await settleReact(p);
      assert.equal(await p.evaluate(()=>window.retainedPreviewNode.isConnected&&window.retainedPreviewNode===document.querySelector('[aria-label="运行窗口预览"]')),true);
      assert.equal(await preview.locator('img').count(),1);assert.equal(await preview.locator('img').getAttribute('src'),before.url);assert.equal(await p.evaluate(()=>window.uiHarness.previewDetaches.length),before.detaches);assert.equal(await p.getByText('尚无可核验的关联窗口',{exact:true}).count(),0);
      assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.filter(v=>v.action==='stop').length),0);assert.deepEqual(await p.evaluate(()=>window.uiHarness.state.session),before.session);assert.deepEqual(await p.evaluate(()=>[...window.uiHarness.previewProjections.values()].at(-1)),before.projection);await screenshot(p,'typed-transient-binding-retains-real-decoded-frame');
    }finally{await p.close();}
  });
  await check('RBC-83-initial-binding-rejection-never-reuses-an-old-game-frame',async()=>{
    const p=await appPage('main',1040,760,port=>port.bindingResponses.push({error:{code:'capture_cleanup_pending',stage:'capture',message:'Controlled initial unknown binding',recoverable:true}}));try{await emitSessionFrame(p);await p.evaluate(()=>{const port=window.uiHarness,previous=port.prepareSessionPreview('previous-game-cache','d'.repeat(64),'live');const old={...structuredClone(port.savedPreviewUpdate.frame),leaseId:previous.leaseId,generation:previous.generation,targetIdentityHash:previous.targetIdentityHash};port.previewFrameFixtures.set(`${previous.leaseId}:${previous.generation}`,old);});await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await p.waitForFunction(()=>window.uiHarness.bindingRejected.length===1);await settleReact(p);assert.equal(await p.getByRole('region',{name:'运行窗口预览',exact:true}).locator('img').count(),0);assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isDisabled(),true);assert.equal(await p.evaluate(()=>window.uiHarness.previewAttaches.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length+window.uiHarness.previewControls.length),0);}finally{await p.close();}
  });
  await check('RBC-84-typed-error-stage-and-recoverable-are-not-message-heuristics',async()=>{
    for(const error of [{...typedBindingBusy,stage:'storage'},{...typedBindingBusy,recoverable:false},{...typedBindingBusy,code:'unknown_busy'},'capture_cleanup_pending busy']){const {p}=await establishedReading();try{await p.evaluate(error=>window.uiHarness.bindingResponses.push({error}),error);await p.getByRole('button',{name:'显示游戏',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.bindingRejected.length===1);await settleReact(p);assert.equal(await p.getByRole('region',{name:'运行窗口预览',exact:true}).locator('img').count(),0);assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isDisabled(),true);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);}finally{await p.close();}}
  });
  await check('RBC-85-successful-binding-read-clears-only-own-transient-notice',async()=>{
    const {p,preview}=await establishedReading();try{await p.evaluate(error=>window.uiHarness.bindingResponses.push({error}),typedBindingBusy);await p.getByRole('button',{name:'显示游戏',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.bindingRejected.length===1);await settleReact(p);const image=await preview.locator('img').getAttribute('src');await p.getByRole('button',{name:'重新核对窗口',exact:true}).click();await p.getByRole('button',{name:'重新核对窗口',exact:true}).waitFor({state:'hidden'});assert.equal(await preview.locator('img').getAttribute('src'),image);assert.equal(await p.evaluate(()=>window.uiHarness.previewAttaches.length),1);assert.equal(await p.evaluate(()=>window.uiHarness.previewDetaches.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length+window.uiHarness.previewControls.length),0);}finally{await p.close();}
  });
  await check('RBC-86-authoritative-unbound-and-stale-invalidate-old-frame-immediately',async()=>{
    for(const state of ['unbound','no_window','stale']){const {p,preview}=await establishedReading();try{await p.evaluate(state=>{const port=window.uiHarness;port.bindingFixture={...port.bindingFixture,state,errorCode:state==='stale'?'target_changed':null};},state);await p.getByRole('button',{name:'显示游戏',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.previewDetaches.length===1);assert.equal(await preview.locator('img').count(),0);assert.equal(await p.getByRole('button',{name:'显示游戏',exact:true}).isDisabled(),true);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.length),0);}finally{await p.close();}}
  });
  await check('RBC-87-late-old-session-binding-errors-and-frames-cannot-enter-new-session',async()=>{
    const {p,preview}=await establishedReading();try{
      const genuinelyOld=await p.evaluate(()=>structuredClone(window.uiHarness.savedPreviewUpdate));
      await p.evaluate(error=>window.uiHarness.bindingResponses.push({error,holdKey:'old-B-binding'}),typedBindingBusy);
      await p.getByRole('button',{name:'显示游戏',exact:true}).click();await p.waitForFunction(()=>window.uiHarness.bindingPending.has('old-B-binding'));
      await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={...port.bindingFixture,sessionId:'B-next-session',targetIdentitySha256:'c'.repeat(64),windowTitle:'New B target'};port.prepareSessionPreview('B-next-session','c'.repeat(64));port.publish({session:{...port.state.session,sessionId:'B-next-session'}});});
      await p.waitForFunction(()=>window.uiHarness.previewSessionQueries.includes('B-next-session'));await emitSessionFrame(p);await preview.locator('img').waitFor();
      const image=await preview.locator('img').getAttribute('src');assert.equal(await preview.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);assert.equal(await p.evaluate(()=>window.uiHarness.bindingPending.has('old-B-binding')),true,'B must bind and display its own frame before A settles');assert.match(await preview.textContent(),/New B target/);
      // Hold a new B request too. Releasing A must not clear B's pending token,
      // start duplicate B reads, overwrite B's binding or unmount B's preview.
      await p.evaluate(()=>{const port=window.uiHarness;port.bindingResponses.push({value:structuredClone(port.bindingFixture),holdKey:'current-B-binding'});});await p.clock.install();await p.clock.runFor(5500);await p.waitForFunction(()=>window.uiHarness.bindingPending.has('current-B-binding'));
      const beforeLate=await p.evaluate(()=>({queries:window.uiHarness.bindingQueries,settled:window.uiHarness.bindingSettled,rejected:window.uiHarness.bindingRejected.length}));
      await p.evaluate(update=>{window.uiHarness.releaseBinding('old-B-binding');update.frame.sequence='18446744073709551615';window.uiHarness.emitPreview(update);},genuinelyOld);await p.waitForFunction(count=>window.uiHarness.bindingRejected.length===count+1,beforeLate.rejected);await settleReact(p);
      assert.equal(await preview.locator('img').getAttribute('src'),image);assert.equal(await p.evaluate(()=>window.uiHarness.bindingPending.has('current-B-binding')),true);assert.equal(await p.getByText('尚无可核验的关联窗口',{exact:true}).count(),0);assert.equal(await p.getByRole('button',{name:'重新核对窗口',exact:true}).count(),0);
      await p.clock.runFor(5500);assert.equal(await p.evaluate(()=>window.uiHarness.bindingQueries),beforeLate.queries,'Old A finally cannot clear B single-flight while B is still pending');
      await p.evaluate(()=>window.uiHarness.releaseBinding('current-B-binding'));await p.waitForFunction(count=>window.uiHarness.bindingSettled===count+2,beforeLate.settled);await settleReact(p);assert.equal(await preview.locator('img').getAttribute('src'),image);assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length+window.uiHarness.previewControls.length),0);
    }finally{await p.close();}
  });
  await check('RBC-88-same-target-new-collector-incarnation-rejects-old-lease-generation',async()=>{
    const {p,preview}=await establishedReading();try{await p.clock.install();const previous=await p.evaluate(()=>structuredClone(window.uiHarness.savedPreviewUpdate));await p.evaluate(()=>window.uiHarness.prepareSessionPreview(window.uiHarness.state.session.sessionId,'a'.repeat(64)));await p.clock.runFor(3500);await p.waitForFunction(()=>window.uiHarness.previewAttaches.length===2);assert.equal(await preview.locator('img').count(),0);await emitSessionFrame(p);await preview.locator('img').waitFor();const image=await preview.locator('img').getAttribute('src');await p.evaluate(update=>{update.frame.sequence='18446744073709551615';window.uiHarness.emitPreview(update);},previous);await settleReact(p);assert.equal(await preview.locator('img').getAttribute('src'),image);assert.equal(await p.evaluate(()=>window.uiHarness.previewAttaches.length),2);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.length),0);}finally{await p.close();}
  });
  await check('RBC-89-real-adapter-same-context-two-subscribers-both-receive-later-frames',async()=>{
    const fixture=await adapterFixture(),a=[],b=[];try{fixture.attach(update=>a.push(update));fixture.attach(update=>b.push(update));await waitUntil(()=>a.length>=2&&b.length>=2);assert.ok(a.some(v=>v.frame.sequence==='2'));assert.ok(b.some(v=>v.frame.sequence==='2'));assert.equal(fixture.maxInFlight,1);assert.equal(fixture.calls.some(v=>v.name!=='application_preview_latest'),false);}finally{fixture.close();}
    // Actual adapter cache/pending microtask race: resolve the next frame before
    // attaching B. The cache task must neither duplicate nor reorder that frame.
    const race=await adapterFixture(),aRace=[],bRace=[];let resolveNext;
    try { race.attach(update=>aRace.push(update));await waitUntil(()=>aRace.length===1);race.replies.push(()=>new Promise(resolve=>{resolveNext=resolve;}));await waitUntil(()=>!!resolveNext);const update=race.update();resolveNext(update);race.attach(value=>bRace.push(value));await waitUntil(()=>bRace.some(value=>value.frame.sequence===update.frame.sequence));await Promise.resolve();await Promise.resolve();assert.equal(bRace.filter(value=>value.frame.sequence===update.frame.sequence).length,1);assert.ok(bRace.every((value,index)=>index===0||BigInt(value.frame.sequence)>BigInt(bRace[index-1].frame.sequence)));assert.equal(race.maxInFlight,1); } finally { race.close(); }
  });
  await check('RBC-90-unique-unsubscribe-is-idempotent-and-never-cancels-new-subscriber',async()=>{
    const fixture=await adapterFixture(),a=[],b=[],c=[];try{const offA=fixture.attach(v=>a.push(v)),offB=fixture.attach(v=>b.push(v));await waitUntil(()=>a.length>=2&&b.length>=2);offA();const frozen=a.length;fixture.attach(v=>c.push(v));offA();await waitUntil(()=>b.length>=3&&c.length>=2);assert.equal(a.length,frozen);offB();const frozenB=b.length;await waitUntil(()=>c.length>=3);assert.equal(b.length,frozenB);assert.equal(fixture.maxInFlight,1);assert.equal(fixture.calls.some(v=>v.name==='application_preview_control'),false);}finally{fixture.close();}
  });
  await check('RBC-91-single-flight-pull-and-late-finally-do-not-delete-new-context',async()=>{
    for(const reject of [false,true]){const fixture=await adapterFixture(),received=[],old=[];let release;try{fixture.replies.push(()=>new Promise((resolve,fail)=>{release=()=>reject?fail(Error('Controlled old transport error')):resolve(fixture.update());}));const off=fixture.attach(v=>old.push(v));await waitUntil(()=>!!release);off();fixture.projection={...fixture.projection,generation:'2'};fixture.attach(v=>received.push(v));assert.equal(fixture.calls.length,1);assert.equal(fixture.maxInFlight,1);release();await waitUntil(()=>received.length>=2);assert.equal(old.length,0);assert.ok(received.every(v=>v.projection.generation==='2'));assert.equal(fixture.maxInFlight,1);assert.ok(fixture.calls.filter(v=>v.args.generation==='2').length>=2);assert.equal(fixture.calls.filter(v=>v.args.generation==='2')[0].args.afterSequence,null);
      let invalidProcessed=0;for(const kind of ['duplicate','backwards','lease','generation','target'])fixture.replies.push(()=>{const update=fixture.update();update.frame.contentHash='f'.repeat(64);if(kind==='duplicate')update.frame.sequence=received.at(-1).frame.sequence;if(kind==='backwards')update.frame.sequence='0';if(kind==='lease')update.frame.leaseId='wrong-lease';if(kind==='generation')update.frame.generation='0';if(kind==='target')update.frame.targetIdentityHash='f'.repeat(64);invalidProcessed++;return update;});
      await waitUntil(()=>invalidProcessed===5);await waitUntil(()=>received.at(-1).frame.contentHash!=='f'.repeat(64)&&fixture.replies.length===0&&Number(received.at(-1).frame.sequence)>=fixture.sequence,5000);assert.equal(received.some(v=>v.frame.contentHash==='f'.repeat(64)),false);assert.equal(fixture.maxInFlight,1);
    }finally{fixture.close();}}
    await assertUnsettledPreviewContext(actualAdapter, waitUntil);
  });
  await check('RBC-92-subscription-transport-failure-is-not-authoritative-target-loss-or-live',async()=>{
    const p=await previewProbePage('reading-adapter');try{await emitSessionFrame(p);const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await preview.locator('img').waitFor();const image=await preview.locator('img').getAttribute('src'),imageTime=await previewTimeText(preview),metadataBefore=await p.evaluate(()=>window.uiHarness.previewSessionQueries.length);await p.evaluate(()=>{const port=window.uiHarness,projection=port.previewProjections.get(port.savedPreviewUpdate.projection.leaseId);projection.capturedAtMs='1792368009000';});await p.waitForFunction(count=>window.uiHarness.previewSessionQueries.length>count,metadataBefore);await settleReact(p);assert.equal(await preview.locator('img').getAttribute('src'),image);assert.equal(await previewTimeText(preview),imageTime,'Metadata alone cannot advance the displayed JPEG capture time');await p.evaluate(()=>window.previewNativeFailure={code:'preview_transport',stage:'capture',message:'Controlled transport read failure',recoverable:true});await p.locator('[data-preview-state="stale"]').waitFor();const metadata=await p.evaluate(()=>window.uiHarness.previewSessionQueries.length);await p.waitForFunction(count=>window.uiHarness.previewSessionQueries.length>count,metadata);await settleReact(p);assert.equal(await preview.getAttribute('data-preview-state'),'stale','Metadata cannot label a still-failed subscription Live');assert.equal(await preview.locator('img').getAttribute('src'),image);assert.match(await preview.locator('img').getAttribute('alt'),/非实时/);assert.match(await preview.textContent(),/最后成功画面/);await p.evaluate(()=>{window.previewNativeFailure=null;const port=window.uiHarness,frame=port.savedPreviewUpdate.frame;frame.sequence='2';frame.capturedAtMs='1792368001002';port.emitPreview({...port.savedPreviewUpdate,frame});});await p.locator('[data-preview-state="live"]').waitFor();const freshTime=new Date(1792368001002).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3});await preview.locator('time').filter({hasText:freshTime}).waitFor();assert.notEqual(await preview.locator('img').getAttribute('src'),image);assert.notEqual(await previewTimeText(preview),imageTime,'Only an accepted new frame advances the image time');assert.equal(await p.evaluate(()=>window.previewNativeCalls.some(v=>['application_preview_start','application_preview_control'].includes(v.name))),false);await screenshot(p,'actual-adapter-transport-recovery-nonlive-last-frame');}finally{await p.close();}await assertStaleCacheSubscription('A');await assertCapabilityOnlyDenial('reading-adapter');await assertMetadataOnlyPreviewRecovery(actualAdapter, waitUntil);
  });
  await check('RBC-93-paused-retained-frame-survives-metadata-errors-and-own-remount',async()=>{
    const {p,preview}=await establishedReading();try{await preview.getByRole('button',{name:'暂停',exact:true}).click();await preview.locator('img').waitFor();await p.evaluate(()=>{const port=window.uiHarness,read=port.previewSession.bind(port);port.failPreviewMetadata=true;port.previewSession=async()=>{if(port.failPreviewMetadata)throw Error('Controlled paused metadata failure');return read();};});await p.clock.install();await p.clock.runFor(3500);await preview.getByRole('alert').waitFor();assert.equal(await preview.locator('img').count(),1);assert.match(await preview.locator('img').getAttribute('alt'),/非实时/);await p.evaluate(()=>window.uiHarness.failPreviewMetadata=false);await p.getByRole('button',{name:'游戏库',exact:true}).click();await p.getByRole('button',{name:'当前会话',exact:true}).click();await openSessionDataset(p,'正文历史');await preview.locator('img').waitFor();assert.equal(await preview.getAttribute('data-preview-state'),'paused');assert.equal(await p.evaluate(()=>window.uiHarness.previewRequests.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.filter(v=>v.action==='stop').length),0);}finally{await p.close();}
  });
  await check('RBC-94-permission-revocation-selection-expiry-and-target-loss-never-stay-live',async()=>{
    for(const [state,code,permission] of [['permission_revoked','preview_permission_revoked','denied'],['unavailable','preview_selection_expired','granted'],['unavailable','preview_selection_failed','granted'],['target_closed','preview_target_closed','granted']]){const {p,preview}=await establishedReading();try{const old=await p.evaluate(()=>structuredClone(window.uiHarness.savedPreviewUpdate));await p.evaluate(({state,code,permission})=>{const port=window.uiHarness;port.emitPreview({projection:{...port.savedPreviewUpdate.projection,state,permission,errorCode:code},frame:null});},{state,code,permission});await p.locator(`[data-preview-state="${state}"]`).waitFor();assert.equal(await preview.locator('img').count(),0);if(code==='preview_selection_failed'){assert.match(await previewDiagnosticText(p, preview), /preview_selection_failed/);assert.equal(await preview.getByRole('button',{name:'重新选择…',exact:true}).isEnabled(),true);}await p.evaluate(old=>{old.frame.sequence='18446744073709551615';old.frame.capturedAtMs='1792368010000';old.projection.capturedAtMs='1792368010000';window.uiHarness.emitPreview(old);},old);await settleReact(p);assert.equal(await preview.locator('img').count(),0);assert.notEqual(await preview.getAttribute('data-preview-state'),'live');assert.equal(await p.evaluate(()=>window.uiHarness.previewPickerRequests.length),0);assert.equal(await p.evaluate(()=>window.uiHarness.previewControls.length),0);}finally{await p.close();}}await assertCapabilityOnlyDenial('adapter');await assertInitialBoundPreviewEntry();
  });
  await check('RBC-95-listener-exception-and-unresolved-consumer-do-not-own-producer',async()=>{
    const fixture=await adapterFixture(),healthy=[];try{const offThrow=fixture.attach(()=>{throw Error('Controlled listener throw');});fixture.attach(()=>new Promise(()=>{}));fixture.attach(v=>healthy.push(v));fixture.attach(()=>{});assert.throws(()=>fixture.attach(()=>{}),/四个/);await waitUntil(()=>healthy.length>=2);assert.equal(fixture.maxInFlight,1);offThrow();const joined=[];fixture.attach(v=>joined.push(v));await waitUntil(()=>joined.length>=2&&healthy.length>=3);assert.equal(fixture.calls.some(v=>v.name!=='application_preview_latest'),false);fixture.close();await waitUntil(()=>fixture.inFlight===0);const reads=fixture.calls.length;await new Promise(resolve=>setTimeout(resolve,650));assert.equal(fixture.calls.length,reads);const resumed=[];fixture.attach(v=>resumed.push(v));await waitUntil(()=>resumed.length>=1);assert.equal(fixture.calls.at(-1).args.afterSequence,null);}finally{fixture.close();}
  });
  await check('RBC-96-real-component-navigation-and-old-cleanup-preserve-another-subscriber',async()=>{
    const p=await previewProbePage('adapter');try{await emitSessionFrame(p);const a=p.getByTestId('preview-subscription-A'),b=p.getByTestId('preview-subscription-B');await a.locator('img').waitFor();await b.locator('img').waitFor();assert.equal(await a.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);await p.getByRole('button',{name:'切换订阅 A',exact:true}).click();assert.equal(await a.count(),0);const previousImage=await b.locator('img').getAttribute('src');await p.evaluate(()=>{const port=window.uiHarness,update=structuredClone(port.savedPreviewUpdate);update.frame.sequence='2';update.frame.capturedAtMs='1792368002000';update.projection.capturedAtMs='1792368002000';port.emitPreview(update);});await p.waitForFunction(image=>document.querySelector('[data-testid="preview-subscription-B"] img')?.src!==image,previousImage);const image=await b.locator('img').getAttribute('src');assert.notEqual(image,previousImage,'B must receive a new frame while A is unmounted');await p.getByRole('button',{name:'切换订阅 A',exact:true}).click();await a.locator('img').waitFor();assert.equal(await b.locator('img').getAttribute('src'),image);await p.getByRole('button',{name:'切换订阅 A',exact:true}).click();await p.getByRole('button',{name:'切换订阅 B',exact:true}).click();await settleReact(p);const reads=await p.evaluate(()=>window.previewNativeCalls.filter(v=>v.name==='application_preview_latest').length);await p.waitForTimeout(650);assert.equal(await p.evaluate(()=>window.previewNativeCalls.filter(v=>v.name==='application_preview_latest').length),reads);assert.equal(await p.evaluate(()=>window.previewNativeCalls.some(v=>v.name==='application_preview_control')),false);}finally{await p.close();}await assertStaleCacheSubscription('B');
  });
  await check('RBC-97-natural-binding-metadata-and-frame-poll-overlap-preserves-context',async()=>{
    const p=await previewProbePage('reading-adapter');try{
      await emitSessionFrame(p);const preview=p.getByRole('region',{name:'运行窗口预览',exact:true});await preview.locator('img').waitFor();const genuinelyOld=await p.evaluate(()=>structuredClone(window.uiHarness.savedPreviewUpdate));
      await p.evaluate(error=>window.uiHarness.bindingResponses.push({error,holdKey:'natural-old-A-binding'}),typedBindingBusy);await p.waitForFunction(()=>window.uiHarness.bindingPending.has('natural-old-A-binding'),null,{timeout:7000});
      await p.evaluate(()=>{const port=window.uiHarness;port.bindingFixture={...port.bindingFixture,sessionId:'natural-new-B-session',targetIdentitySha256:'c'.repeat(64),windowTitle:'Natural new B target'};port.prepareSessionPreview('natural-new-B-session','c'.repeat(64));port.publish({session:{...port.state.session,sessionId:'natural-new-B-session'}});});
      await p.waitForFunction(()=>window.uiHarness.previewSessionQueries.includes('natural-new-B-session'));await emitSessionFrame(p);await preview.locator('img').waitFor();assert.equal(await preview.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);assert.equal(await p.evaluate(()=>window.uiHarness.bindingPending.has('natural-old-A-binding')),true,'A remains unsettled while B already has a decoded frame');
      const retainedImage=await preview.locator('img').getAttribute('src'),retainedTime=await previewTimeText(preview),metadataBefore=await p.evaluate(()=>window.uiHarness.previewSessionQueries.length);await p.evaluate(()=>{const port=window.uiHarness,projection=port.previewProjections.get(port.savedPreviewUpdate.projection.leaseId);projection.capturedAtMs='1792368009000';});await p.waitForFunction(count=>window.uiHarness.previewSessionQueries.length>count,metadataBefore);await settleReact(p);assert.equal(await preview.locator('img').getAttribute('src'),retainedImage);assert.equal(await previewTimeText(preview),retainedTime,'Overlapping metadata does not redate the existing B JPEG');
      const before=await p.evaluate(()=>{window.naturalPreviewNode=document.querySelector('[aria-label="运行窗口预览"]');window.previewNativeAutoSequence=true;window.previewNativeSequence=1;const port=window.uiHarness;return {binding:port.bindingQueries,settled:port.bindingSettled,rejected:port.bindingRejected.length,metadata:port.previewSessionQueries.length,projection:structuredClone([...port.previewProjections.values()].at(-1))};});
      await p.evaluate(error=>{const port=window.uiHarness;port.bindingResponses.push({error},{value:structuredClone(port.bindingFixture)});},typedBindingBusy);
      await p.waitForFunction(before=>window.uiHarness.bindingSettled>=before.settled+2&&window.uiHarness.bindingRejected.length===before.rejected+1,before,{timeout:16000});await settleReact(p);
      assert.equal(await p.evaluate(()=>window.uiHarness.bindingPending.has('natural-old-A-binding')),true,'Two natural B binding cycles must complete without releasing A');assert.equal(await p.evaluate(()=>window.naturalPreviewNode.isConnected&&window.naturalPreviewNode===document.querySelector('[aria-label="运行窗口预览"]')),true);assert.equal(await preview.locator('img').count(),1);assert.equal(await preview.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);assert.ok(await p.evaluate(()=>window.previewNativeSequence)>2);assert.ok(await p.evaluate(()=>window.uiHarness.previewSessionQueries.length)>before.metadata+1);assert.notEqual(await preview.locator('img').getAttribute('src'),retainedImage);assert.notEqual(await previewTimeText(preview),retainedTime,'A real new B frame, rather than metadata, updates the shown capture time');
      const lateCount=await p.evaluate(()=>window.uiHarness.bindingRejected.length);await p.evaluate(update=>{window.uiHarness.releaseBinding('natural-old-A-binding');update.frame.sequence='18446744073709551615';window.uiHarness.emitPreview(update);},genuinelyOld);await p.waitForFunction(count=>window.uiHarness.bindingRejected.length===count+1,lateCount);await settleReact(p);assert.equal(await p.evaluate(()=>window.naturalPreviewNode.isConnected&&window.naturalPreviewNode===document.querySelector('[aria-label="运行窗口预览"]')),true);assert.equal(await preview.locator('img').evaluate(async img=>{await img.decode();return img.naturalWidth;}),160);
      await p.waitForFunction(()=>{const f=window.previewNativeLastFrame;if(!f||f.targetIdentityHash!=='c'.repeat(64))return false;const expected=new Date(Number(f.capturedAtMs)).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false,fractionalSecondDigits:3});return document.querySelector('[aria-label="运行窗口预览"] time')?.textContent.includes(expected);});assert.equal(await p.getByText('尚无可核验的关联窗口',{exact:true}).count(),0);assert.equal(await p.evaluate(()=>window.previewNativeCalls.some(v=>['application_preview_start','application_preview_control','application_preview_select_window'].includes(v.name))),false);assert.equal(await p.evaluate(()=>[...window.uiHarness.previewProjections.values()].at(-1).leaseId),before.projection.leaseId);await screenshot(p,'natural-binding-metadata-frame-poll-overlap');
    }finally{await p.close();}
  });

  await check('RBC-98-real-adapter-unresolved-target-first-frame-binds-once', async () => {
    const initial = { leaseId: 'controlled-unresolved-native-lease', generation: '1', consumerId: 'controlled-unresolved-owner', state: 'starting', permission: 'granted', targetIdentityHash: null, attemptedAtMs: null, capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: null, errorCode: null, detail: 'Explicit unresolved Rust-lease fixture; no native capture occurred' };
    const target = 'c'.repeat(64), calls = [], replies = [], a = [], b = [], c = [], cancellations = [];
    let inFlight = 0, maxInFlight = 0, processed = 0;
    const update = (sequence = '1') => ({ projection: { ...initial, state: 'live', targetIdentityHash: target, capturedAtMs: '1792368001000' }, frame: { leaseId: initial.leaseId, generation: initial.generation, sequence, targetIdentityHash: target, capturedAtMs: '1792368001000', width: 160, height: 100, contentHash: 'e'.repeat(64), encoding: 'jpeg', payload: [1, 2, 3] } });
    const port = await actualAdapter(async (name, args) => {
      calls.push({ name, args }); assert.equal(name, 'application_preview_latest'); inFlight++; maxInFlight = Math.max(maxInFlight, inFlight);
      try { if (replies.length) { const reply = replies.shift()(); processed++; return reply; } return { projection: { ...initial }, frame: null }; } finally { inFlight--; }
    });
    try {
      cancellations.push(port.attachWindowPreview(initial, value => a.push(value)), port.attachWindowPreview(initial, value => b.push(value)));
      await waitUntil(() => a.length > 0 && b.length > 0);
      const invalid = ['metadata', 'lease', 'generation', 'consumer', 'frame-lease', 'frame-generation', 'frame-target', 'encoding', 'zero-sequence', 'bad-sequence', 'empty-payload', 'dimensions', 'permission', 'state', 'malformed-target'];
      for (const kind of invalid) replies.push(() => {
        const value = update();
        if (kind === 'metadata') value.frame = null;
        if (kind === 'lease') value.projection.leaseId = 'other-lease';
        if (kind === 'generation') value.projection.generation = '2';
        if (kind === 'consumer') value.projection.consumerId = 'other-owner';
        if (kind === 'frame-lease') value.frame.leaseId = 'other-lease';
        if (kind === 'frame-generation') value.frame.generation = '2';
        if (kind === 'frame-target') value.frame.targetIdentityHash = 'd'.repeat(64);
        if (kind === 'encoding') value.frame.encoding = 'png';
        if (kind === 'zero-sequence') value.frame.sequence = '0';
        if (kind === 'bad-sequence') value.frame.sequence = '01';
        if (kind === 'empty-payload') value.frame.payload = [];
        if (kind === 'dimensions') value.frame.width = 0;
        if (kind === 'permission') value.projection.permission = 'denied';
        if (kind === 'state') value.projection.state = 'unavailable';
        if (kind === 'malformed-target') value.frame.targetIdentityHash = value.projection.targetIdentityHash = 'not-a-target-digest';
        return value;
      });
      await waitUntil(() => processed === invalid.length, 15000);
      assert.equal(a.filter(value => value.frame).length, 0); assert.equal(b.filter(value => value.frame).length, 0);
      assert.ok(a.every(value => value.projection.targetIdentityHash === null)); assert.ok(b.every(value => value.projection.targetIdentityHash === null), 'Metadata-only and malformed frames must leave the target unresolved');
      replies.push(() => update());
      await waitUntil(() => a.some(value => value.frame) && b.some(value => value.frame));
      assert.equal(a.at(-1).frame.targetIdentityHash, target); assert.equal(b.at(-1).frame.sequence, '1');
      const reads = calls.length;
      // A listener holding the original pending projection joins the same
      // resolved pull; it cannot discard A/B or roll back the native identity.
      cancellations.push(port.attachWindowPreview(initial, value => c.push(value)));
      await waitUntil(() => c.some(value => value.frame));
      assert.equal(c.at(-1).frame.sequence, '1'); assert.equal(calls.length, reads); assert.ok(a.every(value => value.projection.errorCode !== 'preview_generation_conflict')); assert.ok(b.every(value => value.projection.errorCode !== 'preview_generation_conflict'));
      assert.throws(() => port.attachWindowPreview({ ...initial, targetIdentityHash: 'd'.repeat(64) }, () => {}), /目标身份/);
      const counts = [a, b, c].map(values => values.filter(value => value.frame).length), before = processed;
      for (const kind of ['changed-known-target', 'duplicate', 'backwards', 'wrong-owner']) replies.push(() => { const value = update(); if (kind === 'changed-known-target') value.projection.targetIdentityHash = value.frame.targetIdentityHash = 'd'.repeat(64); if (kind === 'duplicate') value.frame.sequence = '1'; if (kind === 'backwards') value.frame.sequence = '0'; if (kind === 'wrong-owner') value.projection.consumerId = 'other-owner'; return value; });
      await waitUntil(() => processed === before + 4); assert.deepEqual([a, b, c].map(values => values.filter(value => value.frame).length), counts);
      replies.push(() => update('2')); await waitUntil(() => [a, b, c].every(values => values.at(-1)?.frame?.sequence === '2'));
      assert.equal(maxInFlight, 1); assert.ok(calls.every(value => value.name === 'application_preview_latest')); assert.ok([a, b, c].every(values => values.filter(value => value.frame).every(value => value.frame.targetIdentityHash === target)));
    } finally { cancellations.forEach(off => off()); }
    // Reverse the race: A is still reading its unresolved lease while B has
    // already read Rust's known target metadata. B cannot replace A's pull.
    for (const expectedHash of [target, 'd'.repeat(64)]) {
      const a = [], b = [], calls = []; let release, sequence = 0, pending = 0, maximum = 0;
      const racing = await actualAdapter(async (name, args) => { calls.push({ name, args }); assert.equal(name, 'application_preview_latest'); pending++; maximum = Math.max(maximum, pending); try { const value = update(String(++sequence)); if (calls.length === 1) await new Promise(resolve => { release = resolve; }); return value; } finally { pending--; } });
      const offA = racing.attachWindowPreview(initial, value => a.push(value)); let offB = () => {};
      try {
        await waitUntil(() => !!release); offB = racing.attachWindowPreview({ ...initial, targetIdentityHash: expectedHash }, value => b.push(value)); assert.equal(calls.length, 1); assert.equal(a.length, 0);
        release(); await waitUntil(() => a.filter(value => value.frame).length >= 2);
        assert.equal(a.some(value => value.projection.errorCode === 'preview_generation_conflict'), false); assert.equal(maximum, 1);
        if (expectedHash === target) { assert.ok(b.filter(value => value.frame).length >= 2); assert.ok(b.every(value => value.projection.targetIdentityHash === target)); }
        else assert.equal(b.length, 0, 'A mismatched known subscriber must neither receive A frames nor block A');
        assert.ok(calls.every(value => value.name === 'application_preview_latest'));
      } finally { offA(); offB(); release?.(); }
    }
  });
  await check('RBC-99-real-component-unresolved-candidate-and-session-decode-first-frame', async () => {
    for (const mode of ['candidate-adapter', 'adapter']) {
      const p = await previewProbePage(mode, port => { const initial = [...port.previewProjections.values()].at(-1); initial.targetIdentityHash = null; initial.state = 'starting'; initial.capturedAtMs = null; });
      try {
        const a = p.getByTestId('preview-subscription-A'), b = p.getByTestId('preview-subscription-B');
        await p.waitForFunction(() => window.previewNativeCalls.some(value => value.name === 'application_preview_latest'));
        assert.equal(await a.locator('img').count(), 0); assert.equal(await b.locator('img').count(), 0);
        const before = await p.evaluate(() => ({ metadata: window.uiHarness.previewSessionQueries.length, session: structuredClone(window.uiHarness.state.session), current: structuredClone(window.uiHarness.state.current) }));
        await p.evaluate(() => { const projection = [...window.uiHarness.previewProjections.values()].at(-1); projection.targetIdentityHash = 'c'.repeat(64); });
        await emitSessionFrame(p);
        for (const preview of [a, b]) { await preview.locator('img').waitFor(); assert.equal(await preview.locator('img').evaluate(async image => { await image.decode(); return image.naturalWidth; }), 160); assert.equal(await preview.getByRole('region', { name: '运行窗口预览', exact: true }).getAttribute('data-preview-state'), 'live'); }
        assert.equal(await p.evaluate(() => window.uiHarness.previewSessionQueries.length), before.metadata, 'The first decoded frame must not depend on a later metadata refresh');
        let image = await b.locator('img').getAttribute('src');
        if (mode === 'adapter') {
          // A metadata read begun before the first frame can arrive after it.
          // It must not clear the already resolved, decoded same-generation image.
          await p.evaluate(() => { const port = window.uiHarness, read = port.previewSession.bind(port); port.lateUnresolvedMetadata = false; port.previewSession = async () => { if (!port.lateUnresolvedMetadata) { port.lateUnresolvedMetadata = true; const result = await read(); return { ...result, state: 'starting', targetIdentityHash: null, capturedAtMs: null }; } return read(); }; });
          await p.waitForFunction(() => window.uiHarness.lateUnresolvedMetadata, null, { timeout: 5000 }); await settleReact(p);
          assert.equal(await b.locator('img').getAttribute('src'), image); assert.equal(await b.getByRole('region', { name: '运行窗口预览', exact: true }).getAttribute('data-preview-state'), 'live');
        }
        const firstPayload = await p.evaluate(() => [...window.uiHarness.savedPreviewUpdate.frame.payload]);
        await p.evaluate(async () => { const port = window.uiHarness, canvas = document.createElement('canvas'); canvas.width = 160; canvas.height = 100; const context = canvas.getContext('2d'); context.fillStyle = '#805c94'; context.fillRect(0, 0, 160, 100); const jpeg = await new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(Error('Controlled second JPEG encoding failed')), 'image/jpeg')); const payload = [...new Uint8Array(await jpeg.arrayBuffer())], digest = await crypto.subtle.digest('SHA-256', new Uint8Array(payload)); const update = structuredClone(port.savedPreviewUpdate); update.frame = { ...update.frame, sequence: '2', capturedAtMs: '1792368002000', contentHash: [...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, '0')).join(''), payload }; update.projection.capturedAtMs = update.frame.capturedAtMs; port.savedPreviewUpdate = update; port.emitPreview(update); });
        await p.waitForFunction(previous => document.querySelector('[data-testid="preview-subscription-B"] img')?.src !== previous, image);
        assert.notDeepEqual(await p.evaluate(() => window.uiHarness.savedPreviewUpdate.frame.payload), firstPayload);
        const secondTime = new Date(1792368002000).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false, fractionalSecondDigits: 3 });
        for (const preview of [a, b]) { assert.equal(await preview.locator('img').evaluate(async value => { await value.decode(); return value.naturalWidth; }), 160); assert.equal(await previewTimeText(preview), `画面时间 ${secondTime}`); }
        image = await b.locator('img').getAttribute('src');
        await p.getByRole('button', { name: '切换订阅 A', exact: true }).click(); await p.getByRole('button', { name: '切换订阅 A', exact: true }).click(); await a.locator('img').waitFor();
        assert.equal(await b.locator('img').getAttribute('src'), image); assert.equal(await a.locator('img').evaluate(async value => { await value.decode(); return value.naturalWidth; }), 160);
        assert.deepEqual(await p.evaluate(() => ({ session: window.uiHarness.state.session, current: window.uiHarness.state.current })), { session: before.session, current: before.current });
        assert.equal(await p.evaluate(() => window.previewNativeCalls.some(value => ['application_preview_start', 'application_preview_select_window', 'application_preview_control'].includes(value.name))), false);
        await screenshot(p, `first-native-frame-unresolved-${mode}`);
      } finally { await p.close(); }
    }
  });
  await check('RBC-100-no-successful-frame-timeout-never-claims-an-expired-image', async () => {
    const p = await previewProbePage('candidate-adapter', port => { const projection = [...port.previewProjections.values()].at(-1); projection.targetIdentityHash = null; projection.state = 'stale'; projection.errorCode = 'preview_selection_timeout'; projection.detail = 'Controlled no-first-frame timeout'; });
    try {
      const preview = p.getByTestId('preview-subscription-A'); await preview.getByText('尚无可用画面', { exact: true }).first().waitFor(); assert.equal(await preview.getByText('画面已过期', { exact: true }).count(), 0); assert.equal(await preview.locator('img').count(), 0); assert.match(await preview.locator('time').textContent(), /尚无成功画面/); assert.equal(await preview.getByRole('button', { name: '重新选择…', exact: true }).isEnabled(), true); await screenshot(p, 'first-frame-timeout-no-expired-image-claim');
    } finally { await p.close(); }
    const failed = await previewProbePage('candidate-adapter', port => { const projection = [...port.previewProjections.values()].at(-1); projection.targetIdentityHash = null; projection.state = 'stale'; projection.errorCode = 'preview_selection_failed'; projection.detail = '诊断：阶段 picker_finished · 系统回调 received=1 processed=1'; });
    try { const preview = failed.getByTestId('preview-subscription-A'); assert.match(await preview.textContent(), /系统未(?:能)?确认这次窗口共享/); assert.match(await previewDiagnosticText(failed, preview), /阶段 picker_finished/); assert.match(await previewDiagnosticText(failed, preview), /received=1 processed=1/); assert.equal(await preview.locator('img').count(), 0); await screenshot(failed, 'first-frame-selection-failure-preserves-native-safe-detail'); } finally { await failed.close(); }
    const retained = await establishedReading();
    try { await retained.p.evaluate(() => { const port = window.uiHarness; port.emitPreview({ projection: { ...port.savedPreviewUpdate.projection, state: 'stale', errorCode: 'preview_capture_timeout', detail: 'Controlled later capture timeout' }, frame: null }); }); await retained.preview.getByText('画面已过期', { exact: true }).first().waitFor(); assert.equal(await retained.preview.locator('img').count(), 1); assert.equal(await retained.preview.getByText('尚无可用画面', { exact: true }).count(), 0); } finally { await retained.p.close(); }
  });

} catch (error) { errors.push(error.stack ?? String(error)); }
finally { await browser?.close(); child?.kill('SIGTERM'); strictChild?.kill('SIGTERM'); for (const process of additionalChildren) process.kill('SIGTERM'); }
const sourceAfter = Object.fromEntries(await Promise.all(auditedFiles.sort().map(async file=>[path.relative(root,file),hash(await fs.readFile(file))])));
if (JSON.stringify(sourceBefore)!==JSON.stringify(sourceAfter)) errors.push('Product UI or test sources changed during this run; rerun after source freeze.');
if (productionSecurityPolicy.violations.length) errors.push('Actual document rendering violated the production Content Security Policy.');
try {
  if (hash(await fs.readFile(path.join(root, securityConfigFile))) !== productionSecurityPolicy.sourceSha256) errors.push('Production security configuration changed during this run.');
  if (hash(await fs.readFile(manifestPath)) !== currentSessionFixtures.fixtureManifestSha256) errors.push('SQLite projection fixture manifest changed during this run.');
  for (const input of fixtureInputs) if (hash(await fs.readFile(path.join(root,input.fileRootRelative))) !== input.sha256) errors.push(`SQLite projection scenario changed during this run: ${input.scenario}`);
  if (hash(await fs.readFile(readingHistoryFile)) !== readingHistoryEntry.sha256) errors.push('SQLite reading history/status payload changed during this run.');
  for (const [file, expected] of Object.entries(actualReadingHistory.sourceSha256)) if (hash(await fs.readFile(path.join(root,file))) !== expected) errors.push(`SQLite reading history/status source changed during this run: ${file}`);
  for (const [file, expected] of Object.entries(fixtureSourceFiles)) if (hash(await fs.readFile(path.join(root,file))) !== expected) errors.push(`SQLite fixture producer source changed during this run: ${file}`);
} catch (error) { errors.push(`Cannot reverify SQLite projection inputs: ${error.message}`); }
const result = { schemaVersion: 1, nonce, assertionCount, currentSessionFixtures, productionSecurityPolicy, additionalTestBuilds, sourceSha256:sourceBefore, layer: 'component', status: errors.length === 0 && rows.length > 0 && rows.every(row => row.status === 'passed') ? 'passed' : 'failed', nativeOrGameVerified: false, visualPixelAcceptance: false, checks: rows, captures, errors };
await fs.writeFile(path.join(out, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ status: result.status, checks: rows, errors, output: out }, null, 2));
if (result.status !== 'passed') process.exitCode = 1;
