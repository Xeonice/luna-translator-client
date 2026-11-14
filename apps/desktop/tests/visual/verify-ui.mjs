import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { assertCaptureConditions } from './capture-conditions.mjs';
import { captureCatalogueState, loadReferenceCatalogue } from './capture-catalogue.mjs';
import { captureReviewedStates } from './capture-reviewed-states.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../../..');
const desktop = path.join(root, 'apps/desktop');
const catalogueIndex = process.argv.indexOf('--reference-catalogue');
const proposalPreview = process.argv.includes('--proposal-preview');
const referenceCatalogue = catalogueIndex >= 0 ? await loadReferenceCatalogue(process.argv[catalogueIndex + 1], { root, proposalPreview }) : null;
if (proposalPreview && !referenceCatalogue) throw new Error('--proposal-preview requires an explicit reference catalogue');
const captureOnlyIndex=process.argv.indexOf('--capture-only');
const captureOnly=captureOnlyIndex>=0?new Set(process.argv[captureOnlyIndex+1].split(',')):null;
if(captureOnly&&!proposalPreview)throw new Error('Targeted captures require --proposal-preview; they cannot pass product acceptance');
const require = createRequire(path.join(root, 'package.json'));
const { chromium } = require('playwright-core');
const { PNG } = require('pngjs');
const pixelmatch = require('pixelmatch').default;
const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
const supplied = process.argv.indexOf('--output');
const out = supplied >= 0 ? path.resolve(process.argv[supplied + 1]) : path.join(root, '.build/product-ui-candidates', timestamp);
if (fsSync.existsSync(out)) throw new Error(`Output already exists; approved or previous evidence will not be overwritten: ${out}`);
await fs.mkdir(out, { recursive: true });
const hash = data => createHash('sha256').update(data).digest('hex');
async function filesUnder(directory) { const found = []; for (const entry of await fs.readdir(directory, { withFileTypes: true })) { const current = path.join(directory, entry.name); if (entry.isDirectory()) found.push(...await filesUnder(current)); else if (entry.isFile()) found.push(current); } return found; }
const nativeWindowSource = 'apps/desktop/src-tauri/src/platform/window_manager.rs';
const nativeInitialConfig = 'apps/desktop/src-tauri/tauri.conf.json';
const nativeSource = await fs.readFile(path.join(root, nativeWindowSource), 'utf8');
const nativeUrlTemplate = nativeSource.match(/format!\(\s*"([^"]*index\.html\?[^"]*)"/)?.[1];
if (!nativeUrlTemplate || !nativeUrlTemplate.includes('{label}')) throw new Error('Native window URL producer could not be inspected; refusing a guessed frontend role test.');
const sourceFiles = [path.join(root, nativeWindowSource), path.join(root, nativeInitialConfig), ...await filesUnder(path.join(desktop, 'src')), ...await filesUnder(path.join(desktop, 'tests')), ...['index.html', 'package.json', 'package-lock.json', 'tsconfig.json', 'vite.config.ts'].map(name => path.join(desktop, name))].sort();
const sourceSha256 = Object.fromEntries(await Promise.all(sourceFiles.map(async file => [path.relative(root, file), hash(await fs.readFile(file))])));
const productionCsp = JSON.parse(await fs.readFile(path.join(root, nativeInitialConfig), 'utf8')).app.security.csp;
assert.equal(typeof productionCsp, 'string'); assert.ok(productionCsp.length > 0);
const productionSecurityPolicy = { sourceFile: nativeInitialConfig, sourceSha256: sourceSha256[nativeInitialConfig], csp: productionCsp, delivery: 'document_response_header', violations: [] };
const designFiles = (await filesUnder(path.join(root, 'docs/design/first-product-v1'))).filter(file => /\.(?:html|js|css|json|png)$/.test(file));
const designSha256 = Object.fromEntries(await Promise.all(designFiles.map(async file => [path.relative(root, file), hash(await fs.readFile(file))])));
const server = createServer(); await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); const port = server.address().port; await new Promise(resolve => server.close(resolve));
async function build(argv, logName) { let log = `Command: ${process.execPath} ${argv.join(' ')}\n`; const processBuild = spawn(process.execPath, argv, { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] }); processBuild.stdout.on('data', chunk => { log += chunk; }); processBuild.stderr.on('data', chunk => { log += chunk; }); const code = await new Promise(resolve => processBuild.on('exit', resolve)); log += `\nExit code: ${code}\n`; await fs.writeFile(path.join(out, logName), log); if (code !== 0) throw new Error(`${logName} failed (${code}): ${log}`); }
await build([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typescript-tests.log');
const productionBuild = path.join(out, 'production-build');
await build([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--outDir', productionBuild], 'production-build.log');
const staged = path.join(out, 'test-build');
await build([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(here, 'vite.config.ts'), '--outDir', staged], 'controlled-build.log');
// Serve an unchanged production bundle through a path-adjusted HTML copy; no test transport is injected.
const smokeDirectory = path.join(staged, 'production-smoke');
await fs.cp(productionBuild, smokeDirectory, { recursive: true });
const smokeHtml = path.join(smokeDirectory, 'index.html');
await fs.writeFile(smokeHtml, (await fs.readFile(smokeHtml, 'utf8')).replaceAll('/assets/', '/production-smoke/assets/'));
const child = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--outDir', staged, '--host', '127.0.0.1', '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
let serverLog = ''; child.stdout.on('data', chunk => { serverLog += chunk; }); child.stderr.on('data', chunk => { serverLog += chunk; });
const url = `http://127.0.0.1:${port}`;
const behavior = []; const captures = []; const errors = [];
let browser;
let browserVersion = null;
async function check(id, fn) { try { await fn(); behavior.push({ id, status: 'passed' }); console.log(id + ': passed'); } catch (error) { behavior.push({ id, status: 'failed', message: error.message }); console.log(id + ': failed: ' + error.message); } }
async function ready() { for (let n = 0; n < 120; n++) { if (child.exitCode !== null) throw new Error(`Vite exited ${child.exitCode}: ${serverLog}`); try { if ((await fetch(`${url}/tests/visual/index.html`)).ok) return; } catch {} await new Promise(resolve => setTimeout(resolve, 100)); } throw new Error('Vite startup timeout'); }
const roleSizes = {};
for (const match of nativeSource.matchAll(/WindowRole::\w+\s*=>\s*\("(main|settings|hud|learning)",\s*"[^"]*",\s*([\d.]+),\s*([\d.]+)\)/g)) roleSizes[match[1]] = { width: Number(match[2]), height: Number(match[3]) };
assert.equal(Object.keys(roleSizes).length, 4, 'All actual native default window sizes must be inspectable');
const initialMain = JSON.parse(await fs.readFile(path.join(root, nativeInitialConfig), 'utf8')).app.windows.find(window => window.label === 'main');
assert.deepEqual({ width: initialMain.width, height: initialMain.height }, roleSizes.main, 'First launch and reopened main windows must use the same design-sized native viewport');
async function navigateWithProductionCsp(page, targetUrl) {
  page.on('console', message => { if (message.type() === 'error' && message.text().includes('Content Security Policy')) productionSecurityPolicy.violations.push({ url: page.url(), message: message.text() }); });
  await page.route('**/*', async route => {
    if (route.request().resourceType() !== 'document') return route.continue();
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': productionCsp } });
  });
  const response = await page.goto(targetUrl);
  assert.equal(await response.headerValue('content-security-policy'), productionCsp, 'Actual document response must enforce the unchanged production CSP');
}
async function pageFor(role = 'main', scenario = 'empty', size = roleSizes[role], extra = '') {
  const page = await browser.newPage({ viewport: size, deviceScaleFactor: 1 }); page.setDefaultTimeout(10000);
  page.on('pageerror', error => errors.push(error.message));
  await navigateWithProductionCsp(page, `${url}/tests/visual/${nativeUrlTemplate.replace('{label}', role)}&scenario=${scenario}${extra}`);
  await page.locator('#root').getByText('正在连接本机应用核心', { exact: true }).waitFor({ state: 'hidden' });
  await page.evaluate(() => window.uiHarness.prepareBindingJourneyFixture());
  await page.waitForFunction(() => !!window.uiHarness && document.getElementById('root').children.length > 0);
  await page.evaluate(() => { Object.assign(document.getElementById('root').style, { left: '0', top: '0', width: '100%', height: '100%' }); });
  await page.waitForLoadState('networkidle'); await page.evaluate(() => document.fonts.ready); return page;
}
// Actual visible shared C04 interaction. Never program the hidden autofill select.
async function selectValue(trigger, value) {
  await trigger.click();
  const page = trigger.page();
  const options = page.getByRole('option');
  const index = await options.evaluateAll((nodes, target) => nodes.findIndex(node => node.getAttribute('data-select-value') === target), value);
  assert.ok(index >= 0, `Visible Select option is missing: ${value}`);
  await options.nth(index).click();
}
async function selectOptions(trigger) {
  await trigger.click();
  const page = trigger.page();
  const options = await page.getByRole('option').evaluateAll(nodes => nodes.map(node => ({ value: node.getAttribute('data-select-value'), label: node.textContent })));
  await page.keyboard.press('Escape');
  return options;
}
async function selectOptionCount(trigger, expected) {
  await trigger.click();
  const page = trigger.page();
  await page.waitForFunction(count => document.querySelectorAll('[role="option"]').length === count, expected);
  const count = await page.getByRole('option').count();
  await page.keyboard.press('Escape');
  return count;
}
// The retired entity tabs are replaced by independent, mounted disclosures.
async function enterConfigurationSection(page, section) {
  const target = page.locator(`details[data-configuration-section="${section}"]`);
  await target.waitFor({ state: 'attached' });
  for (const id of ['service-advanced', 'translation', 'learning', 'testing', 'terms']) {
    const details = page.locator(`details[data-configuration-section="${id}"]`);
    if (await details.getAttribute('open') !== null && id !== section) await details.locator(':scope > summary').click();
  }
  if (await target.getAttribute('open') === null) await target.locator(':scope > summary').click();
}
async function chooseConfigurationService(page, modelId) { await page.locator(`[data-service-model="${modelId}"]`).click(); }
async function chooseConfigurationConnection(page, connectionId) {
  const orphan = page.locator(`[data-service-connection="${connectionId}"]`);
  if (await orphan.count()) await orphan.click();
  else {
    const modelId = await page.evaluate(id => window.uiHarness.state.configuration.draft.models.find(model => model.connectionId === id)?.modelId, connectionId);
    assert.ok(modelId, 'A saved shared connection must be reachable from a real service');
    await chooseConfigurationService(page, modelId);
  }
}
async function enterConfigurationRuntime(page,target=null){
  await enterConfigurationSection(page,'testing');
  // Schedule/context remain in their one authoritative purpose editor.
  const translation=page.locator('details[data-configuration-section="translation"]');
  if(await translation.getAttribute('open')===null)await translation.locator(':scope > summary').click();
  if(target)await selectValue(page.getByRole('combobox',{name:'测试对象',exact:true}), target);
}
async function verifyFailedConfigurationTestRecovery() {
  // Keep this within UI-05: real production components, explicitly prepared
  // transport outcomes. Rust integration tests own actual commit/head CAS.
  const recovery = await pageFor('settings', 'configuration-lifecycle');
  await enterConfigurationRuntime(recovery,'purpose');
  const baseline = await recovery.evaluate(() => ({ snapshots: window.uiHarness.snapshots, head: window.uiHarness.state.configuration.draftHeadRevision, applied: window.uiHarness.state.configuration.appliedHeadRevision, commit: window.uiHarness.state.commitRevision }));
  await recovery.evaluate(() => { const port = window.uiHarness; port.configurationTestResponses.push({ patch: { commitRevision: port.state.commitRevision + 1 }, error: 'UI controlled invalid_structured_output' }); });
  await recovery.getByRole('button', { name: '测试当前对象', exact: true }).click();
  await recovery.getByRole('alert').filter({ hasText: 'UI controlled invalid_structured_output' }).waitFor();
  await recovery.waitForFunction(before => window.uiHarness.snapshots > before, baseline.snapshots);
  assert.equal(await recovery.evaluate(() => window.uiHarness.commands.length), 0, 'testing must not save/apply/enable');
  await recovery.getByRole('spinbutton', { name: '请求超时秒数', exact: true }).fill('61');
  await recovery.getByRole('button', { name: '保存草稿', exact: true }).click();
  await recovery.getByText('草稿已保存', { exact: true }).waitFor();
  const saved = await recovery.evaluate(() => window.uiHarness.commands.at(-1));
  assert.equal(saved.expectedRevision, baseline.commit + 1, 'save uses refreshed global commit');
  assert.equal(saved.command.payload.expectedHeadRevision, baseline.head, 'original editing head is retained');
  assert.equal(await recovery.evaluate(() => window.uiHarness.state.configuration.appliedHeadRevision), baseline.applied);
  await recovery.close();

  const delayed = await pageFor('settings', 'configuration-lifecycle');
  await enterConfigurationRuntime(delayed,'purpose');
  const delayedHead = await delayed.evaluate(() => { const port = window.uiHarness; port.configurationTestResponses.push({ holdKey: 'editing-during-test', patch: { commitRevision: port.state.commitRevision + 1 }, error: 'UI controlled network failure' }); return port.state.configuration.draftHeadRevision; });
  await delayed.getByRole('button', { name: '测试当前对象', exact: true }).click();
  await delayed.waitForFunction(() => window.uiHarness.configurationTestPending.has('editing-during-test'));
  await delayed.getByRole('spinbutton', { name: '请求超时秒数', exact: true }).fill('67');
  await delayed.evaluate(() => window.uiHarness.releaseConfigurationTest('editing-during-test'));
  await delayed.getByRole('alert').filter({ hasText: 'UI controlled network failure' }).waitFor();
  assert.equal(await delayed.getByRole('spinbutton', { name: '请求超时秒数', exact: true }).inputValue(), '67');
  assert.equal(await delayed.evaluate(() => window.uiHarness.commands.length), 0);
  await delayed.getByRole('button', { name: '保存草稿', exact: true }).click();
  await delayed.getByText('草稿已保存', { exact: true }).waitFor();
  const delayedSave = await delayed.evaluate(() => window.uiHarness.commands.at(-1));
  assert.equal(delayedSave.command.payload.expectedHeadRevision, delayedHead);
  assert.equal(delayedSave.command.payload.configuration.bindings.find(binding => binding.purpose === 'translation' && binding.scopeKey === 'global').schedule.timeoutMs, 67000);
  assert.equal(delayedSave.command.payload.configuration.bindings.find(binding => binding.purpose === 'grammar' && binding.scopeKey === 'global').schedule.timeoutMs, 60000);
  await delayed.close();

  const conflict = await pageFor('settings', 'configuration-lifecycle');
  await enterConfigurationRuntime(conflict,'purpose');
  const conflictBasis = await conflict.evaluate(() => { const port = window.uiHarness; const basis = { head: port.state.configuration.draftHeadRevision, commit: port.state.commitRevision }; port.configurationTestResponses.push({ holdKey: 'concurrent-draft', patch: { commitRevision: basis.commit + 2, configuration: { ...structuredClone(port.state.configuration), draftHeadRevision: basis.head + 1 } }, error: 'UI controlled failed test' }); return basis; });
  await conflict.getByRole('button', { name: '测试当前对象', exact: true }).click();
  await conflict.waitForFunction(() => window.uiHarness.configurationTestPending.has('concurrent-draft'));
  await conflict.getByRole('spinbutton', { name: '请求超时秒数', exact: true }).fill('68');
  await conflict.evaluate(() => window.uiHarness.releaseConfigurationTest('concurrent-draft'));
  await conflict.getByRole('alert').filter({ hasText: 'UI controlled failed test' }).waitFor();
  await conflict.evaluate(() => { window.uiHarness.failNextCommand = true; });
  await conflict.getByRole('button', { name: '保存草稿', exact: true }).click();
  await conflict.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor();
  const rejected = await conflict.evaluate(() => window.uiHarness.commands.at(-1));
  assert.equal(rejected.expectedRevision, conflictBasis.commit + 2);
  assert.equal(rejected.command.payload.expectedHeadRevision, conflictBasis.head, 'refresh must not adopt a concurrently changed draft head');
  assert.equal(await conflict.getByRole('spinbutton', { name: '请求超时秒数', exact: true }).inputValue(), '68');
  assert.equal(await conflict.evaluate(() => window.uiHarness.commands.length), 1, 'no automatic save retry');
  assert.equal(await conflict.evaluate(() => window.uiHarness.configurationTests.length), 1);
  await conflict.close();

  const local = await pageFor('settings', 'configuration-lifecycle');
  await enterConfigurationRuntime(local,'purpose');
  const localBasis = await local.evaluate(() => { const port = window.uiHarness; const basis = { head: port.state.configuration.draftHeadRevision, commit: port.state.commitRevision }; port.configurationTestResponses.push({ patch: { commitRevision: basis.commit + 1 }, error: 'UI controlled local validation failure' }); return basis; });
  await local.getByRole('button', { name: '本地校验', exact: true }).click();
  await local.getByRole('alert').filter({ hasText: 'UI controlled local validation failure' }).waitFor();
  assert.equal(await local.evaluate(() => window.uiHarness.configurationTests[0].objectKind), 'local');
  await local.getByRole('spinbutton', { name: '请求超时秒数', exact: true }).fill('63');
  await local.getByRole('button', { name: '保存草稿', exact: true }).click();
  await local.getByText('草稿已保存', { exact: true }).waitFor();
  const localSave = await local.evaluate(() => window.uiHarness.commands.at(-1));
  assert.equal(localSave.expectedRevision, localBasis.commit + 1);
  assert.equal(localSave.command.payload.expectedHeadRevision, localBasis.head);
  await local.close();

  const refreshFailure = await pageFor('settings', 'configuration-lifecycle');
  await enterConfigurationRuntime(refreshFailure,'purpose');
  await refreshFailure.getByRole('button', { name: '测试当前对象', exact: true }).click();
  await refreshFailure.getByText('UI transport does not call a real service', { exact: true }).waitFor();
  await refreshFailure.evaluate(() => { const port = window.uiHarness; port.configurationTestResponses.push({ error: 'UI controlled original model failure' }); port.snapshotFailures.push('UI controlled snapshot recovery failure'); });
  await refreshFailure.getByRole('button', { name: '测试当前对象', exact: true }).click();
  await refreshFailure.getByRole('alert').filter({ hasText: 'UI controlled original model failure' }).waitFor();
  await refreshFailure.getByRole('alert').filter({ hasText: 'UI controlled snapshot recovery failure' }).waitFor();
  assert.equal(await refreshFailure.getByText('UI transport does not call a real service', { exact: true }).count(), 0, 'old test result cleared when a new test begins');
  assert.equal(await refreshFailure.evaluate(() => window.uiHarness.commands.length), 0);
  await refreshFailure.close();
}
async function screenshot(page, id, reference, { selector = '#root', state = null, limitation = null, requiredConditions = null } = {}) { if(captureOnly&&!captureOnly.has(id))return; if (referenceCatalogue) { captures.push(await captureCatalogueState({ page, id, legacyReference: reference, catalogue: referenceCatalogue, root, out })); return; } const expectedConditions = { ...captureContract(id), ...requiredConditions }; const verifiedConditions = await assertCaptureConditions(page, expectedConditions); if (expectedConditions.configurationSection) { const actualSection = await page.locator(`details[data-configuration-section="${expectedConditions.configurationSection}"]`).evaluate(node => ({ section: node.dataset.configurationSection, open: node.open })); assert.deepEqual(actualSection, { section: expectedConditions.configurationSection, open: true }, 'Actual workspace disclosure replaces the retired entity-tab precondition'); assert.equal(await page.getByRole('group', { name: '模型配置实体', exact: true }).count(), 0, 'Retired six-tab navigation must really be absent'); verifiedConditions.configurationSection = actualSection; } const candidate = `${id}.candidate.png`; const viewport = page.viewportSize(); const bounds = await page.locator(selector).boundingBox(); await page.locator(selector).screenshot({ path: path.join(out, candidate), animations: 'disabled' }); const referencePath = path.join(root, reference); const referenceBytes = await fs.readFile(referencePath); const baseline = `${id}.baseline.png`; await fs.writeFile(path.join(out, baseline), referenceBytes); const expected = PNG.sync.read(referenceBytes); const actual = PNG.sync.read(await fs.readFile(path.join(out, candidate))); const width = Math.max(expected.width, actual.width); const height = Math.max(expected.height, actual.height); const before = new PNG({ width, height }); const after = new PNG({ width, height }); before.data.fill(0); after.data.fill(0); PNG.bitblt(expected, before, 0, 0, expected.width, expected.height, 0, 0); PNG.bitblt(actual, after, 0, 0, actual.width, actual.height, 0, 0); const diff = new PNG({ width, height }); const differentPixels = pixelmatch(before.data, after.data, diff.data, width, height, { threshold: .1, includeAA: true }); const dimensionsMatch = expected.width === actual.width && expected.height === actual.height; const ratio = differentPixels / (width * height); const diffName = `${id}.diff.png`; await fs.writeFile(path.join(out, diffName), PNG.sync.write(diff)); captures.push({ id, captureConditions: { viewport, selector, bounds, state, limitation, verifiedConditions, deviceScaleFactor: 1 }, status: dimensionsMatch && differentPixels === 0 ? 'passed' : 'failed', reference, referenceSha256: hash(referenceBytes), baselineCopy: baseline, candidate, candidateSha256: hash(await fs.readFile(path.join(out, candidate))), diff: diffName, baselineCopySha256: hash(referenceBytes), diffSha256: hash(await fs.readFile(path.join(out, diffName))), pixelmatchThreshold: .1, includeAA: true, maxRatio: 0, differentPixels, ratio, expectedSize: [expected.width, expected.height], actualSize: [actual.width, actual.height], reason: dimensionsMatch ? 'Exact comparison retained; layout and content require review.' : 'Frozen design reference and native component canvas have different geometry; no resizing, masking, or threshold relaxation applied.' }); }
function captureContract(id) {
  const base = { theme: 'dark', transparency: 'material', preferences: { sourceFontPt: 18, translationFontPt: 24, segmentationEnabled: true, showRuby: true, showPos: true, readingDisplayMode: 'both', hudBackgroundOpacity: .72 } };
  const fields = (role, name, value) => ({ role, name, value });
  const native = (windowRole, viewport = roleSizes[windowRole]) => ({ ...base, windowRole, viewport, ...(windowRole === 'hud' ? { preferences: {...base.preferences,hudBackgroundOpacity:.82}, hudOpacity:.82, hudControls: { segmentation: true, ruby: true, pos: true, pin: true } } : {}) });
  const localSettings = tab => ({ ...native('settings', { width: 1000, height: 720 }), settingsCategory: '翻译服务', selectedEntityTab: null, configurationSection: tab === '用途' ? 'translation' : 'service-advanced', configuration: { draftHeadRevision: 0, appliedHeadRevision: 0, draftEnabled: [{ purpose: 'translation', enabled: false }], appliedPresent: false } });
  switch (id) {
    case 'empty-library': return { ...native('main'), gameCount: 0, current: null, sourceTexts: [] };
    case 'add-game': return { ...native('main'), gameCount: 0, fields: [{...fields('textbox','游戏名称',designGame.title),exact:false},fields('textbox','原名',designGame.subtitle),fields('combobox','原文语言','ja'),fields('textbox','目标进程路径',designExe)] };
    case 'target-review': return { ...native('main'), gameCount: 1, sourceTexts: [] };
    case 'library-batch': return { ...native('main'), gameCount: 0, fields: [{...fields('textbox','游戏名称','同名程序 · 1'),exact:false},fields('combobox','原文语言','ja')] };
    case 'settings-connections': return { ...localSettings('用途'), configuration: { draftHeadRevision: 0, appliedHeadRevision: 0, draftEnabled: [{ purpose: 'translation', enabled: true }], appliedPresent: true }, fields: [fields('combobox','主模型配置','design-traditional')] };
    case 'model-scheduling': return { ...localSettings('模型'), fields: [fields('textbox','模型配置名称','UI model A'),fields('textbox','实际模型 ID','ui-entered-A')] };
    case 'connection-proxy': return { ...localSettings('连接'), fields: [fields('textbox','连接名称','UI configured service'),fields('textbox','服务地址','http://127.0.0.1:1234/v1')] };
    case 'reading-hud': return { ...native('hud',{width:560,height:172}),hudFitHeight:172,hudMenu:'closed',sourceTexts:[designGame.lines[0].source],sourceFont:'18px',current:{sourceText:designGame.lines[0].source,translations:[{text:designGame.lines[0].translation,providerLabel:'Microsoft · 日中翻译'}]} };
    case 'display-preferences': return { ...native('hud',{width:560,height:366}),hudFitHeight:366,transparency:'reduced',preferences:{...base.preferences,sourceFontPt:36,translationFontPt:48,hudBackgroundOpacity:.82},hudMenu:'open',sourceTexts:[designGame.lines[0].source],sourceFont:'36px' };
    case 'learning-fixed': return { ...native('learning'),sourceTexts:[designGame.lines[0].source],current:{sourceText:designGame.lines[1].source},frozen:{sourceText:designGame.lines[0].source,tokenIndex:null} };
    case 'history-earlier': return { ...native('main'),gameCount:0,session:null,sourceTexts:['以前の原文。'] };
    case 'wordcard-native-size': return { ...native('hud',{width:560,height:290}),hudOpacity:.72,preferences:base.preferences,sourceTexts:['新しい原文。'],current:{sourceText:'新しい原文。'},dictionaryHeadwords:[] };
    case 'dictionary-distinct-headwords': return { ...native('hud'),hudOpacity:.72,preferences:base.preferences,sourceTexts:['箸'],current:{sourceText:'箸'},dictionaryHeadwords:['箸','橋'] };
    case 'learning-japanese-linebreak': return { ...native('learning',{width:1040,height:760}),sourceTexts:['放課後、図書館で彼女と待ち合わせをした。'],sourceFont:'18px',frozen:{sourceText:'放課後、図書館で彼女と待ち合わせをした。',tokenIndex:null} };
    case 'glossary-editing': return { ...native('settings'),settingsCategory:'翻译服务',selectedEntityTab:null,configurationSection:'terms',configuration:{draftHeadRevision:5,appliedHeadRevision:0,draftEnabled:[{purpose:'translation',enabled:false}],appliedPresent:false},fields:[fields('textbox','术语表名称','UI scope glossary'),{...fields('combobox','术语作用范围','global'),options:[{value:'global',label:'全局默认'},{value:'game:ui-term-game',label:'UI glossary scope · 仅此游戏'}]},fields('textbox','原词 1','放課後'),fields('textbox','译法 1','放学后')] };
    default: throw new Error(`No declared comparison preconditions for ${id}`);
  }
}
// Human-authored design samples only populate the controlled transport, never the release bundle.
const designContext = { window: {} };
vm.runInNewContext(await fs.readFile(path.join(root, 'docs/design/first-product-v1/data.js'), 'utf8'), designContext);
const designGame = designContext.window.LunaDesignData.games.find(game => game.id === 'sakura');
function designUnit(line) {
  let byte = 0; let utf16 = 0;
  const tokens = line.tokens.map(token => { const byteStart = byte; const utf16Start = utf16; byte += new TextEncoder().encode(token.surface).length; utf16 += token.surface.length; return { surface: token.surface, lemma: token.lemma ?? token.surface, reading: token.reading ?? '', partOfSpeech: token.pos, byteStart, byteEnd: byte, utf16Start, utf16End: utf16, known: true }; });
  assert.equal(tokens.map(token => token.surface).join(''), line.source, 'Design sample token coverage must be exact');
  return { sourceSummary: { sessionId: 'design-sample-session', gameId: designGame.id, sourceKind: 'hook', state: 'active', startedAtMs: Date.UTC(2026, 9, 18, 10, 24), endedAtMs: null }, unitId: `design-sample-${line.id}`, sessionId: 'design-sample-session', gameId: designGame.id, textRevision: 1, sourceText: line.source, sourceHash: hash(line.source), sourceLanguage: designGame.language, createdAtMs: Date.UTC(2026, 9, 18, 10, 24), translations: line.translation ? [{ translationId: `design-sample-${line.id}-translation`, taskId: 'design-sample-translation', text: line.translation, providerLabel: 'Microsoft · 日中翻译', targetLanguage: 'zh-CN', correction: null }] : [], tokens };
}
const designBottle = `/Users/design/Library/Application Support/CrossOver/Bottles/${designGame.bottle}`;
const designExe = `C:\\Program Files\\${designGame.bottle}\\${designGame.exe}`;
const designLibraryGame = { gameId: designGame.id, displayName: designGame.title, originalTitle: designGame.subtitle, sourceLanguage: designGame.language, coverAssetRef: null, rowRevision: 1, target: { environmentKind: 'crossover', environmentKey: `crossover:${designBottle}`, bottleLocator: designBottle, launchPath: designExe, canonicalExePath: designExe, targetPathKey: null, workingDirectory: null, arguments: [], architecture: designGame.arch, verificationState: 'verified' } };
const designTarget = { candidateId: 'design-sample-sakura-window', windowTitle: designGame.title, executablePath: designExe, windowsPid: 384, processCreatedU64: '133000000000000000', architecture: designGame.arch, environmentKey: `crossover:${designBottle}`, bottleLabel: designGame.bottle, hasWindow: true, windowHandle: null, macosHostPid: null, macosWindowId: null };
async function publishDesignReading(page, { fixed = false } = {}) {
  const original = designUnit(designGame.lines[0]); const next = designUnit(designGame.lines[1]);
  await page.evaluate(({ game, original, next, fixed }) => {
    const port = window.uiHarness;
    port.bindingFixture={state:'bound',sessionId:original.sessionId,targetIdentitySha256:'b'.repeat(64),windowTitle:game.displayName,verifiedAtMs:String(original.createdAtMs),errorCode:null,detail:'Explicit independent UI scenario binding; no native capture proof.'};
    port.publish({ ...(document.documentElement.dataset.windowRole==='hud'?{preferences:{...port.state.preferences,hudBackgroundOpacity:.82}}:{}),games: [game], current: fixed ? next : original, history: { units: fixed ? [next, original] : [original], nextCursor: null }, session: { sessionId: original.sessionId, gameId: game.gameId, sourceKind: 'hook', state: 'active', paused: false, translationEnabled: true, sources: [{ sourceId: 'design-sakura-body', hookName: 'Artemis', hookCode: '', encoding: 'UTF-16', sample: original.sourceText, selected: true }] }, ...(fixed ? { learningObject: { unit: original, tokenIndex: null } } : {}) });
  }, { game: designLibraryGame, original, next, fixed });
  await page.getByTestId('source-text').waitFor(); await page.evaluate(() => document.fonts.ready);
}
async function captureSupplementaryTargetStates(){
  if(captureOnly.has('glossary-editing')){
    const config=await pageFor('settings','model-config',{width:1040,height:760});
    await config.evaluate(()=>{const p=window.uiHarness;p.publish({configuration:{...p.state.configuration,draftHeadRevision:3}});});
    await enterConfigurationSection(config,'terms');await config.getByRole('button',{name:'添加术语表',exact:true}).click();
    await config.getByRole('textbox',{name:'术语表名称',exact:true}).fill('UI scope glossary');
    await selectValue(config.getByRole('combobox',{name:'术语作用范围',exact:true}),'game:ui-term-game');
    for(const [index,source,target] of [[1,'図書館','图书馆'],[2,'放課後','放学后']]){await config.getByRole('button',{name:'添加词项',exact:true}).click();await config.getByRole('textbox',{name:`原词 ${index}`,exact:true}).fill(source);await config.getByRole('textbox',{name:`译法 ${index}`,exact:true}).fill(target);}
    await config.getByRole('switch',{name:'启用词项 1',exact:true}).click();await config.getByRole('button',{name:'保存草稿',exact:true}).click();await config.getByText('草稿已保存',{exact:true}).waitFor();
    await config.getByRole('button',{name:'从草稿移除词项 1',exact:true}).click();await selectValue(config.getByRole('combobox',{name:'术语作用范围',exact:true}),'global');await config.getByRole('button',{name:'保存草稿',exact:true}).click();await config.getByText('草稿已保存',{exact:true}).waitFor();
    await screenshot(config,'glossary-editing','docs/design/first-product-v1/previews/22-model-entities.png');await config.close();
  }

  if(captureOnly.has('wordcard-native-size')){
    const hud=await pageFor('hud','reading',{width:560,height:290});await hud.getByRole('button',{name:'放課後，读音 ほうかご',exact:true}).click();
    await hud.getByRole('dialog',{name:'本地词典',exact:true}).getByRole('heading',{name:'放課後',exact:true}).waitFor();
    await hud.evaluate(()=>{const p=window.uiHarness;const next={...p.state.current,unitId:'ui-native-next',sourceText:'新しい原文。',tokens:[]};p.publish({current:next,history:{units:[next,...p.state.history.units],nextCursor:null}});});
    await screenshot(hud,'wordcard-native-size','docs/design/first-product-v1/previews/10-local-word-card.png');await hud.close();
  }
  if(captureOnly.has('learning-japanese-linebreak')){
    const learning=await pageFor('learning','learning',{width:1040,height:760});await learning.evaluate(()=>{const p=window.uiHarness;const unit={...p.state.learningObject.unit,unitId:`unit-${'a'.repeat(64)}`};p.publish({current:unit,learningObject:{unit,tokenIndex:null},history:{units:[unit],nextCursor:null}});});
    await screenshot(learning,'learning-japanese-linebreak','docs/design/first-product-v1/previews/11-learning-unconfigured.png');await learning.close();
  }
  if(captureOnly.has('dictionary-distinct-headwords')){
    const words=await pageFor('hud','reading');
    await words.evaluate(()=>{const p=window.uiHarness;const unit={...p.state.current,sourceText:'箸',tokens:[{surface:'箸',lemma:'箸',reading:'はし',partOfSpeech:'名詞',byteStart:0,byteEnd:3,utf16Start:0,utf16End:1,known:true}]};p.dictionaryEntries=[{entryId:'ui-word-chopsticks',lemma:'箸',reading:'はし',partOfSpeech:['n'],contentKind:'zh_base',sourceLabel:'UI dictionary A',sourceVersion:'ui-v1',licenseRef:'UI attribution license',attribution:'UI attribution A',senses:[{language:'zh',text:'筷子。',usageNotes:'常用于餐具。',sourceLocator:'ui-source-location-a'},{language:'en',text:'chopsticks',usageNotes:JSON.stringify({tags:['arch'],source_sense_id:'ui-private-source-sense'}),sourceLocator:'ui-source-location-a2'}]},{entryId:'ui-word-bridge',lemma:'橋',reading:'はし',partOfSpeech:['n'],contentKind:'zh_base',sourceLabel:'UI dictionary B',sourceVersion:'ui-v2',licenseRef:'UI attribution license B',attribution:'UI attribution B',senses:[{language:'zh',text:'桥；桥梁。',usageNotes:JSON.stringify(['ui-opaque-tag']),sourceLocator:'ui-source-location-b'}]}];p.publish({current:unit,history:{units:[unit],nextCursor:null}});});
    await words.getByRole('button',{name:'箸，读音 はし',exact:true}).click();await words.getByRole('article',{name:'词典词条 箸',exact:true}).getByText('许可与署名',{exact:true}).click();
    await screenshot(words,'dictionary-distinct-headwords','docs/design/first-product-v1/previews/10-local-word-card.png');await words.close();
  }
}
async function captureDesignStates() {
  await captureReviewedStates({catalogue:referenceCatalogue,pageFor,screenshot,publishDesignReading,designLibraryGame,designTarget,selectValue,root,check,captureOnly});
  if(captureOnly && [...captureOnly].every(id=>referenceCatalogue?.states.find(state=>state.id===id)?.requirements?.length))return;
  if(captureOnly){await captureSupplementaryTargetStates();if([...captureOnly].every(id=>['wordcard-native-size','dictionary-distinct-headwords','learning-japanese-linebreak','glossary-editing'].includes(id)))return;}
  if(!captureOnly || captureOnly.has('history-earlier')){
    const history=await pageFor('main','empty');
    await history.getByRole('button',{name:'手动翻译',exact:true}).click();
    await history.getByRole('textbox',{name:'原文',exact:true}).fill('独立した入力。');
    await history.getByRole('button',{name:'翻译并阅读',exact:true}).click();
    await history.getByTestId('source-text').filter({hasText:'独立した入力。'}).filter({visible:true}).waitFor();
    await history.evaluate(()=>{const p=window.uiHarness;const old={...p.state.history.units[0],unitId:'ui-older-unit',sourceText:'以前の原文。',tokens:[]};p.historyPages.older={units:[old],nextCursor:null};p.publish({history:{...p.state.history,nextCursor:'older'}});});
    await history.getByRole('button',{name:'历史记录',exact:true}).click();
    await history.getByRole('button',{name:'加载更早记录',exact:true}).click();
    await history.getByRole('button',{name:'以前の原文。',exact:false}).click();
    await screenshot(history,'history-earlier','docs/design/first-product-v1/previews/20-history.png');
    await history.close();
    if(captureOnly?.size===1)return;
  }
  const library = await pageFor();
  await screenshot(library, 'empty-library', 'docs/design/first-product-v1/previews/38-library-empty.png', { state: 'first-run-empty-library', limitation: 'Reference includes a complete desktop and waiting HUD; candidate is the actual independent main content window.' });
  await library.evaluate(({ game, bottle, exe }) => { window.uiHarness.executableSelections = [{ path: exe, displayName: game.title, architecture: game.arch, environmentKey: `crossover:${bottle}`, bottleLocator: bottle }]; }, { game: designGame, bottle: designBottle, exe: designExe });
  await library.getByRole('button', { name: '添加游戏…', exact: true }).first().click();
  await library.getByRole('button', { name: '选择文件…', exact: true }).click();
  await library.getByRole('textbox', { name: '原名', exact: true }).fill(designGame.subtitle);
  await selectValue(library.getByRole('combobox', { name: '原文语言', exact: true }), designGame.language);
  await library.getByRole('textbox', { name: '目标进程路径', exact: true }).fill(designExe);
  await screenshot(library, 'add-game', 'docs/design/first-product-v1/previews/40-add-game-review.png', { selector: '[data-ui-surface="sheet"]', state: 'program-selected-sakura-metadata-review', limitation: 'Production form geometry/content differences remain visible; file selection was handled by the controlled port.' });
  await library.getByRole('button', { name: '取消', exact: true }).click();
  await library.evaluate(({ game, target }) => { window.uiHarness.targetCandidates = [target];window.uiHarness.bindingJourneyFixture.autoMatchGameId=game.gameId; window.uiHarness.publish({ games: [game] }); }, { game: designLibraryGame, target: designTarget });
  await library.getByRole('button', { name: '连接运行中的游戏…', exact: true }).first().click();
  await library.getByRole('button', { name: '点击选择窗口…', exact: true }).click();
  await screenshot(library, 'target-review', 'docs/design/first-product-v1/previews/42-window-mapping.png', { selector: '[data-ui-surface="sheet"]', state: 'sakura-window-selected-target-review-before-injection', limitation: 'Production review omits the picker list and host mapping shown in the reference; genuine layout/identity-content gap is retained.' });
  await library.close();
  const settings = await pageFor('settings', 'model-config', { width: 1000, height: 720 });
  await settings.evaluate(() => { const port = window.uiHarness; const draft = structuredClone(port.state.configuration.draft); draft.connections[0] = { ...draft.connections[0], connectionId: 'design-microsoft', displayName: 'Microsoft 翻译', protocol: 'azure_translator', deployment: 'cloud', authMode: 'api_key', endpoint: 'https://api.cognitive.microsofttranslator.com', credentialPoolId: null, credentialPoolRevision: null }; draft.models = [{ ...draft.models[0], modelId: 'design-traditional', connectionId: 'design-microsoft', actualModelId: null, displayName: 'Microsoft · 日中翻译' }]; draft.bindings[0] = { ...draft.bindings[0], enabled: true, engines: [{ ...draft.bindings[0].engines[0], modelId: 'design-traditional' }] }; port.publish({ configuration: { ...port.state.configuration, draft, applied: structuredClone(draft) } }); });
  await enterConfigurationSection(settings,'translation');
  await screenshot(settings, 'settings-connections', 'docs/design/first-product-v1/previews/31-dark-settings.png', { state: 'dark-settings-translation-purpose', limitation: 'The historical capture ID is preserved; reference31 is a purpose editor, not a connection editor. Native titlebar is outside the browser content canvas.' });
  await settings.close();
  const hud = await pageFor('hud', 'reading', { width: 560, height: 172 }); await publishDesignReading(hud);
  await screenshot(hud, 'reading-hud', 'docs/design/first-product-v1/previews/09-hud-detail.png', { state: 'translated-sakura-first-line-comfortable-hud' });
  await hud.close();
  const learning = await pageFor('learning', 'learning'); await publishDesignReading(learning, { fixed: true });
  await learning.getByRole('tab', { name: '本句精读', exact: true }).click();
  await learning.getByRole('heading', { name: '为这类理解选择一个模型', exact: true }).waitFor();
  await screenshot(learning, 'learning-fixed', 'docs/design/first-product-v1/previews/11-learning-unconfigured.png', { state: 'sentence-study-unconfigured-fixed-original-newer-current', limitation: 'Reference contains game, HUD and independent learning window on a full desktop; candidate is the design-sized learning content window, not a fabricated desktop composition.' });
  await learning.close();
  const batch = await pageFor();
  await batch.evaluate(() => { window.uiHarness.executableSelections = ['A', 'B'].map((suffix, index) => ({ path: `C:\\Games\\Unknown\\Game.exe`, displayName: `同名程序 · ${index + 1}`, architecture: 'x86', environmentKey: `crossover:/Users/design/Bottles/Unknown-Bottle-${suffix}`, bottleLocator: `/Users/design/Bottles/Unknown-Bottle-${suffix}` })); });
  await batch.getByRole('button', { name: '添加游戏…', exact: true }).first().click(); await batch.getByRole('button', { name: '选择文件…', exact: true }).click();
  await selectValue(batch.getByRole('combobox', { name: '原文语言', exact: true }), 'ja');
  await screenshot(batch, 'library-batch', 'docs/design/first-product-v1/previews/46-custom-batch-review.png', { selector: '[data-ui-surface="sheet"]', state: 'two-same-name-programs-review-before-saving', limitation: 'Production uses the actual multi-item review; remaining visual differences against the historical reference are retained. No completed library is substituted for review.' }); await batch.close();
  const preferences = await pageFor('hud', 'reading', { width: 560, height: 366 }); await publishDesignReading(preferences);
  await preferences.evaluate(() => { const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, transparencyMode: 'reduced', sourceFontPt: 36, translationFontPt: 48 } }); });
  // Reference32 actually contains the open HUD menu. Its original design
  // script closed settings without dismissing the menu; inspect the PNG rather
  // than inferring a closed state from that action name.
  const openLargeHud = { windowRole: 'hud', viewport: { width: 560, height: 366 }, hudMenu: 'open' };
  await check('UI-43-display-reference-rejects-wrong-menu-state', async () => {
    await assert.rejects(() => assertCaptureConditions(preferences, openLargeHud), /HUD menu must be open/);
    await preferences.getByRole('button', { name: '更多阅读操作', exact: true }).click();
    await preferences.getByRole('menu').waitFor();
    assert.equal(await preferences.getByRole('button', { name: '更多阅读操作', exact: true }).getAttribute('aria-expanded'), 'true');
    await assert.rejects(() => assertCaptureConditions(preferences, { ...openLargeHud, hudMenu: 'closed' }), /HUD menu must be closed/);
    await assertCaptureConditions(preferences, openLargeHud);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...openLargeHud, viewport: { width: 500, height: 366 } }), /viewport/);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...openLargeHud, windowRole: 'settings' }), /window role/);
  });
  await check('UI-48-all-state-capture-contract-rejects-wrong-semantic-inputs', async () => {
    const actual = captureContract('display-preferences');
    await assertCaptureConditions(preferences, actual);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...actual, theme:'light' }), /theme/);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...actual, transparency:'material' }), /transparency/);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...actual, preferences:{...actual.preferences,sourceFontPt:18} }), /preferences/);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...actual, sourceTexts:['different original'] }), /sourceTexts/);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...actual, current:{sourceText:'different original'} }), /current.sourceText/);
    await assert.rejects(() => assertCaptureConditions(preferences, { ...actual, sourceFont:'18px' }), /rendered source font/);
  });
  await screenshot(preferences, 'display-preferences', 'docs/design/first-product-v1/previews/32-large-opaque-hud.png', { state: 'large-type-reduced-transparency-hud-menu-open', requiredConditions: openLargeHud }); await preferences.close();
  const model = await pageFor('settings', 'model-config', { width: 1000, height: 720 }); await enterConfigurationSection(model,'service-advanced');
  await screenshot(model, 'model-scheduling', 'docs/design/first-product-v1/previews/22-model-entities.png', { state: 'model-entity-editor-top', limitation: 'Historical capture ID retained; reference22 is model entity editing rather than scheduling.' }); await model.close();
  const connection = await pageFor('settings', 'model-config', { width: 1000, height: 720 }); await enterConfigurationSection(connection,'service-advanced');
  await screenshot(connection, 'connection-proxy', 'docs/design/first-product-v1/previews/23-connection-entities.png', { state: 'connection-entity-editor-top', limitation: 'Reference entity uses a cloud key-auth service; candidate fixture uses a local service. Proxy override has no approved exact reference state.' }); await connection.close();
}

try {
  await ready();
  browser = await chromium.launch({ ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE } : {}), headless: true });
  browserVersion = browser.version();
  if(captureOnly){await captureDesignStates();}else{
  const page = await pageFor();
  await check('UI-01-empty-library-no-fake-game', async () => { assert.equal(await page.getByRole('heading', { name: '把第一款游戏加入这里' }).count(), 1); assert.equal(await page.evaluate(() => window.uiHarness.state.games.length), 0); assert.equal(await page.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); });
  
  await check('UI-02-add-cancel-does-not-inject', async () => { await page.getByRole('button', { name: '添加游戏…', exact: true }).first().click(); await page.getByRole('textbox', { name: '游戏名称', exact: false }).fill('UI controlled game');  await page.getByRole('button', { name: '取消', exact: true }).click(); assert.equal(await page.evaluate(() => window.uiHarness.state.games.length), 0); assert.equal(await page.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); });
  await check('UI-03-association-one-confirmation-explicit-stream', async () => { await page.getByRole('button', { name: '连接运行中的游戏…', exact: true }).first().click(); await page.getByRole('button', { name: '点击选择窗口…' }).click(); assert.equal(await page.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); assert.equal(await page.getByRole('button', { name: '核对目标', exact: true }).count(), 0);  await page.getByRole('button', { name: '关联并接入', exact: true }).click(); await page.getByRole('heading', { name: '正在接入游戏' }).waitFor(); assert.equal(await page.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 1); assert.equal(await page.evaluate(() => window.uiHarness.commands.filter(item => item.command.kind === 'select_source').length), 0); await page.getByRole('button', { name: '关闭面板', exact: true }).click(); });
  await check('UI-04-manual-save-failure-retains-draft-through-navigation', async () => { await page.getByRole('button', { name: '手动翻译', exact: true }).click(); await page.getByRole('textbox', { name: '原文', exact: true }).fill('独立した入力。'); await page.evaluate(() => { window.uiHarness.failNextCommand = true; }); await page.getByRole('button', { name: '翻译并阅读', exact: true }).click(); await page.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor(); assert.equal(await page.getByRole('textbox', { name: '原文', exact: true }).inputValue(), '独立した入力。'); await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.getByRole('button', { name: '手动翻译', exact: true }).click(); assert.equal(await page.getByRole('textbox', { name: '原文', exact: true }).inputValue(), '独立した入力。'); await page.getByRole('button', { name: '翻译并阅读', exact: true }).click(); await page.locator('[data-testid=source-text]:visible').filter({ hasText: '独立した入力。' }).waitFor(); assert.equal(await page.locator('[data-testid=source-text]:visible').count(), 1); assert.equal(await page.evaluate(() => window.uiHarness.state.history.units[0].sourceText), '独立した入力。'); });
  const settings = await pageFor('settings');
  await check('UI-05-settings-draft-test-apply-separated', async () => { await enterConfigurationSection(settings,'service-advanced'); await settings.getByRole('button', { name: '添加服务…', exact: true }).click(); await settings.getByRole('textbox', { name: '新服务名称', exact: true }).fill('UI local draft'); await settings.getByRole('button', { name: '创建服务草稿', exact: true }).click(); await enterConfigurationSection(settings,'service-advanced'); await settings.getByRole('textbox', { name: '连接名称', exact: true }).fill('UI local draft'); await settings.getByRole('textbox', { name: '服务地址', exact: true }).fill('http://127.0.0.1:1234/v1'); await selectValue(settings.getByRole('combobox', { name: '认证方式', exact: true }), 'none'); await settings.getByRole('button', { name: '输入与HOOK', exact: true }).click(); await settings.getByRole('button', { name: '翻译服务', exact: true }).click(); assert.equal(await settings.getByRole('textbox', { name: '连接名称', exact: true }).inputValue(), 'UI local draft'); assert.equal(await settings.evaluate(() => window.uiHarness.commands.length), 0); await settings.getByRole('button', { name: '保存草稿', exact: true }).click(); await settings.getByText('草稿已保存', { exact: true }).waitFor(); assert.equal(await settings.evaluate(() => window.uiHarness.commands.at(-1).command.kind), 'save_configuration'); assert.equal(await settings.evaluate(() => window.uiHarness.state.configuration.applied), null); await verifyFailedConfigurationTestRecovery(); });
  
  await check('UI-06-settings-light-return-intent', async () => { await settings.getByRole('button', { name: '返回主界面', exact: true }).click(); assert.equal(await settings.evaluate(() => window.uiHarness.windows.at(-1)), 'main'); });
  const hud = await pageFor('hud', 'reading');
  
  await check('UI-07-wordcard-fixed-through-new-line', async () => { await hud.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).click(); await hud.getByRole('dialog', { name: '本地词典' }).getByRole('heading', { name: '放課後', exact: true }).waitFor(); await hud.evaluate(() => { const port = window.uiHarness; const next = { ...port.state.current, unitId: 'ui-fixture-unit-2', sourceText: '次の原文。', tokens: [] }; port.publish({ current: next, history: { units: [next, ...port.state.history.units], nextCursor: null } }); }); await hud.getByTestId('source-text').filter({ hasText: '次の原文。' }).waitFor(); assert.equal(await hud.getByRole('dialog', { name: '本地词典' }).getByRole('heading', { name: '放課後', exact: true }).count(), 1); await hud.getByRole('button', { name: '关闭词卡', exact: true }).click(); });
  await check('UI-08-subscription-gap-recovers-with-snapshot', async () => { const before = await hud.evaluate(() => window.uiHarness.snapshots); await hud.evaluate(() => { const port = window.uiHarness; port.publish({ current: { ...port.state.current, unitId: 'ui-fixture-unit-3', sourceText: '差分の回復。', tokens: [] } }, true); }); await hud.getByTestId('source-text').filter({ hasText: '差分の回復。' }).waitFor(); assert.ok(await hud.evaluate(() => window.uiHarness.snapshots) > before); });
  const learning = await pageFor('learning', 'learning');
  await check('UI-09-learning-open-does-not-auto-call-model', async () => { await learning.getByRole('heading', { name: '正在理解的原文', exact: true }).waitFor(); assert.equal(await learning.evaluate(() => window.uiHarness.state.tasks.length), 0); await learning.getByRole('heading', { name: '放課後', exact: true }).waitFor(); });
  
  await check('UI-10-large-font-and-narrow-window-no-horizontal-overflow', async () => { await hud.setViewportSize({ width: 390, height: 844 }); await hud.evaluate(() => { const root = document.getElementById('root'); root.style.width = '100%'; root.style.height = '100%'; root.style.left = '0'; root.style.top = '0'; const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, sourceFontPt: 36, translationFontPt: 48 } }); }); await hud.waitForTimeout(50); const sizes = await hud.evaluate(() => ({ viewport: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth })); assert.equal(sizes.scroll, sizes.viewport); await hud.screenshot({ path: path.join(out, 'hud-large-narrow.candidate.png') }); });
  await check('UI-11-release-bootstrap-excludes-controlled-transport', async () => { const entry = await fs.readFile(path.join(desktop, 'src/app/bootstrap.tsx'), 'utf8'); assert.ok(entry.includes('new TauriApplicationPort()')); assert.ok(!entry.includes('DesignPort')); const bundleFiles = await fs.readdir(path.join(productionBuild, 'assets')); for (const file of bundleFiles.filter(file => file.endsWith('.js'))) { const source = await fs.readFile(path.join(productionBuild, 'assets', file), 'utf8'); for (const marker of ['ui-controlled-fixture', 'ui-fixture-unit', 'uiHarness', 'simulated save failure', 'UI controlled game']) assert.ok(!source.includes(marker), `fixture marker in release: ${marker}`); } });
  const batch = await pageFor();
  await check('UI-12-batch-program-review-preserves-all-and-language-confirmation', async () => { await batch.evaluate(() => { window.uiHarness.executableSelections = [{ path: '/UI-test/First.exe', displayName: 'First controlled game', architecture: 'x86', environmentKey: 'crossover:/UI-test/Bottle', bottleLocator: '/UI-test/Bottle' }, { path: '/UI-test/Second.exe', displayName: 'Second controlled game', architecture: 'x64', environmentKey: 'crossover:/UI-test/Bottle', bottleLocator: '/UI-test/Bottle' }]; }); await batch.getByRole('button', { name: '添加游戏…', exact: true }).first().click(); await batch.getByRole('button', { name: '选择文件…', exact: true }).click(); assert.equal(await batch.getByRole('textbox', { name: '游戏名称', exact: false }).inputValue(), 'First controlled game'); assert.equal(await batch.getByRole('combobox', { name: '原文语言', exact: true }).getAttribute('data-select-value'), ''); await selectValue(batch.getByRole('combobox', { name: '原文语言', exact: true }), 'ja'); await batch.getByRole('button', { name: '核对下一项', exact: true }).click(); await batch.getByRole('textbox', { name: '游戏名称', exact: false }).waitFor(); assert.equal(await batch.getByRole('textbox', { name: '游戏名称', exact: false }).inputValue(), 'Second controlled game'); await selectValue(batch.getByRole('combobox', { name: '原文语言', exact: true }), 'zh-CN'); assert.equal(await batch.evaluate(() => window.uiHarness.commands.length), 0); assert.equal(await batch.evaluate(() => window.uiHarness.state.games.length), 0); await batch.getByRole('button', { name: '添加到游戏库', exact: true }).click(); await batch.getByRole('dialog').waitFor({ state: 'hidden' }); assert.deepEqual(await batch.evaluate(() => window.uiHarness.state.games.map(game => game.sourceLanguage)), ['ja', 'zh-CN']); assert.equal(await batch.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); });
  
  await check('UI-13-native-filedrop-opens-review-not-injection', async () => { await batch.evaluate(() => window.uiHarness.emitDrop(['/UI-test/Dropped.exe'])); await batch.getByRole('textbox', { name: '游戏名称', exact: false }).waitFor(); assert.equal(await batch.getByRole('textbox', { name: '游戏名称', exact: false }).inputValue(), 'Dropped.exe'); assert.equal(await batch.evaluate(() => window.uiHarness.state.games.length), 2); assert.equal(await batch.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0); await batch.getByRole('button', { name: '取消', exact: true }).click(); });
  await check('UI-14-parameter-zero-omit-and-draft-cas-identity', async () => { await enterConfigurationSection(settings,'service-advanced'); await settings.getByRole('textbox', { name: '实际模型 ID', exact: true }).fill('ui-entered-model-id'); await enterConfigurationSection(settings,'service-advanced'); await selectValue(settings.locator('section:visible').filter({has:settings.getByRole('heading',{name:'模型默认参数',exact:true})}).getByRole('combobox', { name: 'Temperature 参数策略', exact: true }), 'set'); assert.equal(await settings.locator('section:visible').filter({has:settings.getByRole('heading',{name:'模型默认参数',exact:true})}).getByRole('spinbutton', { name: 'Temperature 值', exact: true }).inputValue(), '0'); await settings.getByRole('button', { name: '保存草稿', exact: true }).click(); await settings.getByText('草稿已保存', { exact: true }).waitFor(); const zero = await settings.evaluate(() => window.uiHarness.commands.at(-1).command.payload.configuration.models[0].parameters.temperature); assert.deepEqual(zero, { mode: 'set', value: 0 }); await selectValue(settings.locator('section:visible').filter({has:settings.getByRole('heading',{name:'模型默认参数',exact:true})}).getByRole('combobox', { name: 'Temperature 参数策略', exact: true }), 'omit'); const base = await settings.evaluate(() => window.uiHarness.state.configuration.draftHeadRevision); await settings.evaluate(() => { const port = window.uiHarness; port.publish({ configuration: { ...port.state.configuration, draftHeadRevision: port.state.configuration.draftHeadRevision + 5 } }); }); await settings.getByRole('button', { name: '保存草稿', exact: true }).click(); await settings.getByText('草稿已保存', { exact: true }).waitFor(); const sent = await settings.evaluate(() => window.uiHarness.commands.at(-1).command.payload); assert.equal(sent.expectedHeadRevision, base); assert.deepEqual(sent.configuration.models[0].parameters.temperature, { mode: 'omit', value: null }); });
  await check('UI-15-preference-draft-survives-settings-navigation', async () => { await settings.getByRole('button', { name: '显示与快捷键', exact: true }).click(); await settings.getByRole('spinbutton', { name: '原文字号（pt）', exact: true }).fill('32'); await settings.getByRole('button', { name: '输入与HOOK', exact: true }).click(); await settings.getByRole('button', { name: '显示与快捷键', exact: true }).click(); assert.equal(await settings.getByRole('spinbutton', { name: '原文字号（pt）', exact: true }).inputValue(), '32');  });
  await check('UI-16-learning-new-line-and-explicit-ai-request-identity', async () => { const fixed = await learning.evaluate(() => window.uiHarness.state.learningObject.unit.unitId); await learning.evaluate(() => { const port = window.uiHarness; const next = { ...port.state.current, unitId: 'ui-fixed-next', sourceText: '新しい固定対象。', tokens: [] }; port.publish({ current: next, history: { units: [next, ...port.state.history.units], nextCursor: null } }); }); await learning.getByText('新台词已到达。', { exact: false }).waitFor(); assert.equal(await learning.evaluate(() => window.uiHarness.state.learningObject.unit.unitId), fixed); assert.equal(await learning.evaluate(() => window.uiHarness.studies.length), 0); await learning.getByRole('button', { name: '分析最新一句', exact: true }).click(); await learning.getByTestId('source-text').filter({ hasText: '新しい固定対象。' }).waitFor(); assert.equal(await learning.evaluate(() => window.uiHarness.studies.length), 0); await learning.getByRole('button', { name: '分析本句文法', exact: true }).click(); await learning.waitForFunction(() => window.uiHarness.studies.length === 1); assert.equal(await learning.evaluate(() => window.uiHarness.studies[0].unitId), 'ui-fixed-next'); });
  await check('UI-17-history-pagination-retains-earlier-object', async () => { await page.evaluate(() => { const port = window.uiHarness; const old = { ...port.state.history.units[0], unitId: 'ui-older-unit', sourceText: '以前の原文。', tokens: [] }; port.historyPages.older = { units: [old], nextCursor: null }; port.publish({ history: { ...port.state.history, nextCursor: 'older' } }); }); await page.getByRole('button', { name: '历史记录', exact: true }).click(); await page.getByRole('button', { name: '加载更早记录', exact: true }).click(); await page.getByRole('button', { name: '以前の原文。', exact: false }).click(); await page.locator('[data-testid=source-text]:visible').filter({ hasText: '以前の原文。' }).waitFor(); await page.screenshot({ path: path.join(out, 'history-pagination-behavior.candidate.png') }); });
  await check('UI-18-pool-version-and-connection-reference-separated', async () => { await settings.getByRole('button', { name: '翻译服务', exact: true }).click(); await enterConfigurationSection(settings,'service-advanced'); await selectValue(settings.getByRole('combobox', { name: '认证方式', exact: true }), 'api_key'); await settings.evaluate(() => { const port = window.uiHarness; port.publish({ credentials: [{ credentialId: 'ui-cred-a', maskedLabel: 'UI A ·••1111', state: 'active', lifecycleRevision: 1 }, { credentialId: 'ui-cred-b', maskedLabel: 'UI B ·••2222', state: 'active', lifecycleRevision: 1 }] }); }); await selectValue(settings.getByRole('combobox', { name: '密钥选择策略', exact: true }), 'round_robin'); await settings.getByText('加入此池', { exact: true }).nth(0).click(); await settings.getByText('加入此池', { exact: true }).nth(1).click(); assert.equal(await settings.getByRole('switch', { name: '加入此池', exact: true }).nth(0).isChecked(), true); assert.equal(await settings.getByRole('switch', { name: '加入此池', exact: true }).nth(1).isChecked(), true); await settings.getByRole('button', { name: '保存新的池版本', exact: true }).click(); await settings.getByRole('button', { name: '用于此连接草稿', exact: true }).waitFor(); await settings.waitForFunction(() => window.uiHarness.state.credentialPools.length === 1); assert.equal(await settings.evaluate(() => window.uiHarness.state.configuration.draft.connections[0].credentialPoolId), null); const command = await settings.evaluate(() => window.uiHarness.commands.at(-1).command); assert.equal(command.kind, 'save_credential_pool'); assert.equal(command.payload.pool.policy, 'round_robin'); assert.equal(command.payload.pool.members.filter(member => member.enabled).length, 2); await settings.getByRole('button', { name: '用于此连接草稿', exact: true }).click(); assert.equal(await settings.evaluate(() => window.uiHarness.state.configuration.draft.connections[0].credentialPoolId), null); assert.equal(await settings.locator('span:visible').filter({ hasText: /^未保存更改$/ }).count(), 1); });
  const nativeHud = await pageFor('hud', 'reading');
  await check('UI-19-wordcard-fits-native-sized-viewport-after-source-change', async () => { await nativeHud.setViewportSize({ width: 560, height: 290 }); await nativeHud.evaluate(() => { Object.assign(document.getElementById('root').style, { left: '0', top: '0', width: '100%', height: '100%' }); }); await nativeHud.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).click(); const dialog = nativeHud.getByRole('dialog', { name: '本地词典' }); await dialog.getByRole('heading', { name: '放課後', exact: true }).waitFor(); await nativeHud.evaluate(() => { const port = window.uiHarness; const next = { ...port.state.current, unitId: 'ui-native-next', sourceText: '新しい原文。', tokens: [] }; port.publish({ current: next, history: { units: [next, ...port.state.history.units], nextCursor: null } }); }); await nativeHud.getByTestId('source-text').filter({ hasText: '新しい原文。' }).waitFor(); await screenshot(nativeHud, 'wordcard-native-size', 'docs/design/first-product-v1/previews/10-local-word-card.png', { state: 'fixed-wordcard-after-new-source-native560x290', limitation: 'Controlled browser viewport is its simulated work area, so this checks frozen-content and reachable-footer behavior only. The real native card may extend beyond the HUD and uses monitor work area; cross-window bounds and focus require packaged verification. This is not a same-composition visual acceptance.' }); const box = await dialog.evaluate(element => { const viewport = element.parentElement; const bounds = viewport.getBoundingClientRect(); return { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }; }); assert.ok(box.x >= 0 && box.y >= 0 && box.x + box.width <= 560 && box.y + box.height <= 290, JSON.stringify(box)); const open = dialog.getByRole('button', { name: '打开学习窗口', exact: true }); await open.scrollIntoViewIfNeeded(); const footer = await open.boundingBox(); assert.ok(footer && footer.y >= 0 && footer.y + footer.height <= 290, JSON.stringify(footer)); });
  await check('UI-20-native-url-to-production-frontend-role-contract', async () => { for (const role of ['settings', 'hud', 'learning']) { const window = await pageFor(role, role === 'learning' ? 'learning' : role === 'hud' ? 'reading' : 'empty'); assert.equal(await window.evaluate(() => document.documentElement.dataset.windowRole), role); if (role === 'settings') await window.getByRole('heading', { name: '翻译服务', exact: true }).waitFor(); else if (role === 'learning') await window.getByRole('heading', { name: '正在理解的原文', exact: true }).waitFor(); else await window.getByTestId('source-text').waitFor(); assert.equal(await window.getByRole('heading', { name: '游戏库', exact: true }).count(), 0); await window.close(); } });
  const config = await pageFor('settings', 'model-config');
  await check('UI-21-engine-order-and-enable-preserve-identity', async () => { await enterConfigurationSection(config,'translation');
    assert.equal(await config.getByRole('button', { name: '上移引擎 1', exact: true }).isDisabled(), true);
    assert.equal(await config.getByRole('button', { name: '下移引擎 2', exact: true }).isDisabled(), true);
    await config.getByText('启用引擎2', { exact: true }).click();
    await config.getByRole('button', { name: '上移引擎 2', exact: true }).click();
    assert.equal(await config.getByRole('combobox', { name: '引擎 1 模型', exact: true }).getAttribute('data-select-value'), 'ui-model-b');
    assert.equal(await config.getByRole('switch', { name: '启用引擎1', exact: true }).isChecked(), false);
    assert.equal(await config.evaluate(() => window.uiHarness.commands.length), 0);
    await config.getByRole('button', { name: '保存草稿', exact: true }).click();
    await config.getByText('草稿已保存', { exact: true }).waitFor();
    const sent = await config.evaluate(() => window.uiHarness.commands.at(-1).command.payload.configuration.bindings[0]);
    assert.deepEqual(sent.engines.map(engine => [engine.engineId, engine.enabled]), [['ui-engine-b', false], ['ui-engine-a', true]]);
    assert.deepEqual(sent.engines[1].parameters.temperature, { mode: 'set', value: 0 });
    assert.equal(await config.evaluate(() => window.uiHarness.state.configuration.applied), null);
    assert.equal(await config.evaluate(() => window.uiHarness.state.tasks.length), 0);
  });
  await check('UI-22-schedule-defaults-and-boundary-errors', async () => {
    await enterConfigurationRuntime(config,'purpose');
    const save = config.getByRole('button', { name: '保存草稿', exact: true });
    const concurrency = config.getByRole('spinbutton', { name: '用途总并发', exact: true });
    const queue = config.getByRole('spinbutton', { name: '待执行队列容量', exact: true });
    const retries = config.getByRole('spinbutton', { name: '最大重试次数', exact: true });
    assert.equal(await concurrency.inputValue(), '1'); assert.equal(await queue.inputValue(), '32'); assert.equal(await retries.inputValue(), '0');
    for (const value of ['', '0', '9', '1.5']) { await concurrency.fill(value); assert.equal(await save.isDisabled(), true); }
    await concurrency.fill('8');
    for (const value of ['0', '129']) { await queue.fill(value); assert.equal(await save.isDisabled(), true); }
    await queue.fill('128'); await retries.fill('4'); assert.equal(await save.isDisabled(), true);
    await retries.fill('3'); assert.equal(await save.isEnabled(), true); await retries.fill('0');
    await save.click(); await config.getByText('草稿已保存', { exact: true }).waitFor();
    assert.deepEqual(await config.evaluate(() => window.uiHarness.state.configuration.draft.bindings[0].schedule), { timeoutMs: 60000, retries: 0, concurrency: 8, queueCapacity: 128 });
    
  });
  await check('UI-23-proxy-override-preserves-metadata-and-rejects-userinfo', async () => {
    await enterConfigurationSection(config,'service-advanced');
    await selectValue(config.getByRole('combobox', { name: '代理策略', exact: true }), 'override');
    const proxy = config.getByRole('textbox', { name: '独立代理地址', exact: true });
    await proxy.fill('http://name:password@127.0.0.1:7890');
    assert.equal(await config.getByRole('button', { name: '保存草稿', exact: true }).isDisabled(), true);
    await config.getByRole('alert').filter({ hasText: '代理地址不能包含明文凭据' }).waitFor();
    await proxy.fill('http://127.0.0.1:7890');
    await config.getByRole('button', { name: '保存草稿', exact: true }).click(); await config.getByText('草稿已保存', { exact: true }).waitFor();
    assert.deepEqual(await config.evaluate(() => window.uiHarness.state.configuration.draft.connections[0].networkMetadata), { region: 'ui-preserved-region', proxyUrl: 'http://127.0.0.1:7890' });
    await selectValue(config.getByRole('combobox', { name: '代理策略', exact: true }), 'disabled');
    assert.equal(await proxy.count(), 0);
    await selectValue(config.getByRole('combobox', { name: '代理策略', exact: true }), 'override');
    assert.equal(await proxy.inputValue(), 'http://127.0.0.1:7890');
    
  });
  await check('UI-24-game-glossary-draft-failure-and-stable-terms', async () => {
    await enterConfigurationSection(config,'terms');
    await config.getByRole('button', { name: '添加术语表', exact: true }).click();
    await config.getByRole('textbox', { name: '术语表名称', exact: true }).fill('UI scope glossary');
    const scopeContract = captureContract('glossary-editing');
    const scopeField = scopeContract.fields.find(field => field.name === '术语作用范围');
    const scopeOptions = await selectOptions(config.getByRole('combobox', { name: '术语作用范围', exact: true }));
    assert.deepEqual(scopeOptions, scopeField.options);
    await selectValue(config.getByRole('combobox', { name: '术语作用范围', exact: true }), 'game:ui-term-game');
    await config.getByRole('button', { name: '添加词项', exact: true }).click();
    await config.getByRole('textbox', { name: '原词 1', exact: true }).fill('図書館');
    await config.getByRole('textbox', { name: '译法 1', exact: true }).fill('图书馆');
    await config.getByText('启用词项 1', { exact: true }).click();
    await config.getByRole('button', { name: '添加词项', exact: true }).click();
    await config.getByRole('textbox', { name: '原词 2', exact: true }).fill('放課後');
    await config.getByRole('textbox', { name: '译法 2', exact: true }).fill('放学后');
    await config.evaluate(() => { window.uiHarness.failNextCommand = true; });
    await config.getByRole('button', { name: '保存草稿', exact: true }).click();
    await config.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor();
    assert.equal(await config.evaluate(() => window.uiHarness.state.configuration.draft.termSets.length), 0);
    await enterConfigurationSection(config,'service-advanced');
    await enterConfigurationSection(config,'terms');
    assert.equal(await config.getByRole('textbox', { name: '原词 1', exact: true }).inputValue(), '図書館');
    assert.equal(await config.getByRole('switch', { name: '启用词项 1', exact: true }).isChecked(), false);
    await config.getByRole('button', { name: '保存草稿', exact: true }).click(); await config.getByText('草稿已保存', { exact: true }).waitFor();
    const before = await config.evaluate(() => window.uiHarness.state.configuration.draft.termSets[0]);
    assert.equal(before.scopeKey, 'game:ui-term-game'); assert.equal(before.gameId, 'ui-term-game');
    assert.deepEqual(before.terms.map(term => [term.sourceText, term.targetText, term.enabled]), [['図書館', '图书馆', false], ['放課後', '放学后', true]]);
    assert.equal(await config.getByRole('button', { name: '测试当前对象', exact: true }).filter({visible:true}).count(), 0, 'Glossary entity has no object-test action; it cannot issue model calls');
    assert.equal(await config.evaluate(()=>window.uiHarness.configurationTests.length),0);
    await config.getByRole('button', { name: '从草稿移除词项 1', exact: true }).click();
    await selectValue(config.getByRole('combobox', { name: '术语作用范围', exact: true }), 'global');
    await config.getByRole('button', { name: '保存草稿', exact: true }).click(); await config.getByText('草稿已保存', { exact: true }).waitFor();
    const after = await config.evaluate(() => window.uiHarness.state.configuration.draft.termSets[0]);
    assert.equal(after.terms[0].termId, before.terms[1].termId); assert.equal(after.scopeKey, 'global'); assert.equal(after.gameId, null);
    assert.equal(await config.evaluate(() => window.uiHarness.state.configuration.applied), null);
    await screenshot(config, 'glossary-editing', 'docs/design/first-product-v1/previews/22-model-entities.png', { state: 'glossary-editor-after-save', limitation: 'No approved glossary editor reference exists; this preserved comparison against model entity reference22 is intentionally a different-state failure, not visual acceptance.' });
  });
  await check('UI-25-production-entry-actually-mounts-without-test-transport', async () => {
    const production = await browser.newPage(); const pageErrors = [];
    production.on('pageerror', error => pageErrors.push(error.message));
    await navigateWithProductionCsp(production, `${url}/production-smoke/index.html?role=settings`);
    await production.getByText('暂时无法读取应用状态', { exact: true }).waitFor();
    assert.equal(await production.evaluate(() => document.documentElement.dataset.windowRole), 'settings');
    assert.ok(await production.locator('#root > *').count() > 0);
    assert.equal(await production.evaluate(() => 'uiHarness' in window), false);
    assert.deepEqual(pageErrors, []); await production.close();
  });
  await check('UI-26-dictionary-entries-retain-headwords-and-hide-import-metadata', async () => {
    const words = await pageFor('hud', 'reading');
    await words.evaluate(() => {
      const port = window.uiHarness;
      const unit = { ...port.state.current, sourceText: '箸', tokens: [{ surface: '箸', lemma: '箸', reading: 'はし', partOfSpeech: '名詞', byteStart: 0, byteEnd: 3, utf16Start: 0, utf16End: 1, known: true }] };
      port.dictionaryEntries = [
        { entryId: 'ui-word-chopsticks', lemma: '箸', reading: 'はし', partOfSpeech: ['n'], contentKind: 'zh_base', sourceLabel: 'UI dictionary A', sourceVersion: 'ui-v1', licenseRef: 'UI attribution license', attribution: 'UI attribution A', senses: [{ language: 'zh', text: '筷子。', usageNotes: '常用于餐具。', sourceLocator: 'ui-source-location-a' }, { language: 'en', text: 'chopsticks', usageNotes: JSON.stringify({ tags: ['arch'], source_sense_id: 'ui-private-source-sense' }), sourceLocator: 'ui-source-location-a2' }] },
        { entryId: 'ui-word-bridge', lemma: '橋', reading: 'はし', partOfSpeech: ['n'], contentKind: 'zh_base', sourceLabel: 'UI dictionary B', sourceVersion: 'ui-v2', licenseRef: 'UI attribution license B', attribution: 'UI attribution B', senses: [{ language: 'zh', text: '桥；桥梁。', usageNotes: JSON.stringify(['ui-opaque-tag']), sourceLocator: 'ui-source-location-b' }] },
      ];
      port.publish({ current: unit, history: { units: [unit], nextCursor: null } });
    });
    await words.getByRole('button', { name: '箸，读音 はし', exact: true }).click();
    const dialog = words.getByRole('dialog', { name: '本地词典', exact: true });
    const chopsticks = dialog.getByRole('article', { name: '词典词条 箸', exact: true });
    const bridge = dialog.getByRole('article', { name: '词典词条 橋', exact: true });
    await chopsticks.getByRole('heading', { name: '箸', exact: true }).waitFor();
    await bridge.getByRole('heading', { name: '橋', exact: true }).waitFor();
    assert.equal(await chopsticks.getByRole('heading', { name: '橋', exact: true }).count(), 0);
    assert.equal(await bridge.getByRole('heading', { name: '箸', exact: true }).count(), 0);
    assert.equal(await chopsticks.getByText('筷子。', { exact: true }).count(), 1);
    assert.equal(await chopsticks.getByText('桥；桥梁。', { exact: true }).count(), 0);
    assert.equal(await bridge.getByText('桥；桥梁。', { exact: true }).count(), 1);
    assert.equal(await bridge.getByText('筷子。', { exact: true }).count(), 0);
    assert.equal(await chopsticks.getByText('はし', { exact: true }).count(), 1);
    assert.equal(await bridge.getByText('はし', { exact: true }).count(), 1);
    assert.equal(await chopsticks.getByText('常用于餐具。', { exact: true }).count(), 1);
    const text = await dialog.textContent();
    for (const raw of ['source_sense_id', 'ui-private-source-sense', 'ui-opaque-tag', 'tags', 'ui-source-location']) assert.ok(!text.includes(raw), `Raw dictionary metadata leaked: ${raw}`);
    await chopsticks.getByText('许可与署名', { exact: true }).click();
    assert.equal(await chopsticks.getByText('UI attribution A', { exact: false }).isVisible(), true);
    assert.equal(await chopsticks.getByText('UI dictionary A', { exact: false }).isVisible(), true);
    assert.equal(await bridge.getByText('UI dictionary B', { exact: false }).isVisible(), true);
    await screenshot(words, 'dictionary-distinct-headwords', 'docs/design/first-product-v1/previews/10-local-word-card.png', { state: 'distinct-homophone-headwords', limitation: 'Correctness regression uses distinct dictionary entries absent from the approved screenshot; no matching approved visual state exists.' });
    await words.close();
  });
  const typography = await pageFor('learning', 'learning');
  await check('UI-27-learning-hides-internal-unit-identity', async () => {
    await typography.evaluate(() => {
      const port = window.uiHarness;
      const unit = { ...port.state.learningObject.unit, unitId: `unit-${'a'.repeat(64)}` };
      port.publish({ current: unit, learningObject: { unit, tokenIndex: null }, history: { units: [unit], nextCursor: null } });
    });
    await typography.getByRole('heading', { name: '正在理解的原文', exact: true }).waitFor();
    assert.ok(!(await typography.locator('body').innerText()).includes(`unit-${'a'.repeat(64)}`));
    assert.equal(await typography.getByTestId('source-text').count(), 1);
  });
  await check('UI-28-japanese-punctuation-flow-and-200-percent-narrow-layout', async () => {
    for (const width of [1040, 560, 390]) {
      for (const font of [18, 36]) {
        await typography.setViewportSize({ width, height: 900 });
        await typography.evaluate(({ font }) => {
          Object.assign(document.getElementById('root').style, { left: '0', top: '0', width: '100%', height: '100%' });
          const port = window.uiHarness;
          port.publish({ preferences: { ...port.state.preferences, sourceFontPt: font } });
        }, { font });
        await typography.evaluate(() => document.fonts.ready);
        const layout = await typography.getByTestId('source-text').evaluate(element => {
          const tokens = [...element.querySelectorAll('[data-token-index]')];
          const punctuation = tokens.filter(token => !/[\p{L}\p{M}\p{N}]/u.test(token.textContent));
          const last = tokens.at(-1); const preceding = tokens.at(-2);
          const rect = node => { const range = document.createRange(); range.selectNodeContents(node); return [...range.getClientRects()].filter(box => box.width > 0).at(-1).toJSON(); };
          const source = element.getBoundingClientRect();
          return { viewport: document.documentElement.clientWidth, documentWidth: document.documentElement.scrollWidth, sourceWidth: source.width, sourceScrollWidth: element.scrollWidth, punctuation: punctuation.map(token => ({ text: token.textContent, button: token.tagName === 'BUTTON' })), last: rect(last), preceding: rect(preceding.querySelector('ruby').firstChild) };
        });
        assert.ok(layout.punctuation.length >= 2);
        assert.ok(layout.punctuation.every(token => !token.button));
        assert.equal(layout.documentWidth, layout.viewport, JSON.stringify({ width, font, layout }));
        assert.ok(layout.sourceScrollWidth <= Math.ceil(layout.sourceWidth), JSON.stringify({ width, font, layout }));
        assert.ok(Math.abs(layout.last.y - layout.preceding.y) < 2, `Closing punctuation detached at ${width}px/${font}pt: ${JSON.stringify(layout)}`);
      }
    }
    await typography.setViewportSize({ width: 1040, height: 760 });
    await typography.evaluate(() => { const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, sourceFontPt: 18 } }); });
    await screenshot(typography, 'learning-japanese-linebreak', 'docs/design/first-product-v1/previews/11-learning-unconfigured.png', { state: 'sentence-study-japanese-punctuation1040x760', limitation: 'Typography regression uses its controlled sentence in a resized independent window; reference is the full Sakura desktop.' });
    await typography.evaluate(() => {
      const port = window.uiHarness;
      const surfaces = ['「', '図書館', '」、', '（', '学校', '）', '！', '？']; let byte = 0; let utf16 = 0;
      const tokens = surfaces.map(surface => { const byteStart = byte; const utf16Start = utf16; byte += new TextEncoder().encode(surface).length; utf16 += surface.length; return { surface, lemma: surface, reading: '', partOfSpeech: '', byteStart, byteEnd: byte, utf16Start, utf16End: utf16, known: true }; });
      const unit = { ...port.state.learningObject.unit, sourceText: surfaces.join(''), tokens };
      port.publish({ learningObject: { unit, tokenIndex: null }, current: unit, history: { units: [unit], nextCursor: null } });
    });
    await typography.getByRole('button', { name: '図書館', exact: true }).waitFor();
    const clickable = await typography.getByTestId('source-text').getByRole('button').allTextContents();
    assert.deepEqual(clickable, ['図書館', '学校']);
    await typography.getByRole('button', { name: '学校', exact: true }).click();
    await typography.waitForFunction(() => window.uiHarness.state.learningObject.tokenIndex === 4);
  });
  const hudControls = await pageFor('hud', 'reading');
  await hudControls.setViewportSize({ width: 560, height: 290 });
  await hudControls.evaluate(() => { Object.assign(document.getElementById('root').style, { left: '0', top: '0', width: '100%', height: '100%' }); });
  await check('UI-29-hud-menu-opens-after-release-and-stays-open', async () => {
    const trigger = hudControls.getByRole('button', { name: '更多阅读操作', exact: true });
    const menu = hudControls.getByRole('menu', { name: '更多阅读操作', exact: true });
    const box = await trigger.boundingBox(); assert.ok(box);
    await hudControls.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await hudControls.mouse.down();
    assert.equal(await menu.count(), 0, 'Menu must not depend on a held pointer press.');
    await hudControls.mouse.up();
    await menu.waitFor();
    assert.equal(await hudControls.getByRole('dialog', { name: '更多阅读操作', exact: true }).isVisible(), true);
    await hudControls.mouse.move(30, 30);
    await hudControls.evaluate(() => { const port = window.uiHarness; port.publish({ current: { ...port.state.current, sourceText: 'メニュー中にも原文は更新する。', tokens: [] } }); });
    assert.equal(await menu.isVisible(), true);
    assert.equal(await trigger.getAttribute('aria-expanded'), 'true');
    await hudControls.screenshot({ path: path.join(out, 'hud-click-menu.candidate.png') });
    await hudControls.keyboard.press('Escape');
    await menu.waitFor({ state: 'hidden' });
    await hudControls.waitForFunction(() => document.activeElement?.getAttribute('aria-label') === '更多阅读操作');
    await hudControls.keyboard.press('ArrowUp');
    await menu.waitFor();
    assert.equal(await menu.getByRole('menuitem').last().evaluate(element => element === document.activeElement), true);
    await hudControls.keyboard.press('Escape');
    await hudControls.waitForFunction(() => document.activeElement?.getAttribute('aria-label') === '更多阅读操作');
    const before = await hudControls.evaluate(() => window.uiHarness.windows.length);
    await hudControls.keyboard.press('Enter'); await menu.waitFor();
    assert.equal(await menu.getByRole('menuitem', { name: '设置…', exact: true }).evaluate(element => element === document.activeElement), true);
    await hudControls.keyboard.press('Enter');
    await menu.waitFor({ state: 'hidden' });
    assert.equal(await hudControls.evaluate(() => window.uiHarness.windows.length), before + 1);
    assert.equal(await hudControls.evaluate(() => window.uiHarness.windows.at(-1)), 'settings');
    await hudControls.getByRole('button', { name: '分析本句文法', exact: true }).click();
    assert.equal(await hudControls.evaluate(() => window.uiHarness.windows.at(-1)), 'learning');
    await trigger.click(); await menu.waitFor();
    await hudControls.mouse.click(25, 25);
    await menu.waitFor({ state: 'hidden' });
  });
  await check('UI-30-hud-scrolls-only-reading-and-menu-regions', async () => {
    for (const height of [290, 140]) {
      await hudControls.setViewportSize({ width: 560, height });
      await hudControls.evaluate(() => { const port = window.uiHarness; port.publish({ current: { ...port.state.current, sourceText: '長い原文を読む。\n'.repeat(70), tokens: [], translations: [] } }); });
      const reading = hudControls.locator('[data-ui-scroll="reading"]');
      await hudControls.waitForFunction(() => document.querySelector('[data-testid="source-text"]').textContent.split('\n').length >= 70);
      await hudControls.evaluate(() => new Promise(requestAnimationFrame));
      await reading.evaluate(element => { element.scrollTop = 0; });
      const footer = hudControls.locator('footer').first();
      const before = await footer.boundingBox(); assert.ok(before);
      await hudControls.mouse.move(200, 55); await hudControls.mouse.wheel(0, 800);
      await hudControls.waitForFunction(() => document.querySelector('[data-ui-scroll="reading"]').scrollTop > 0, null, { timeout: 5000 }).catch(error => { throw new Error(`Reading wheel at ${height}px: ${error.message}`); });
      const scrolled = await reading.evaluate(element => element.scrollTop); assert.ok(scrolled > 0);
      await reading.evaluate(element => { element.scrollTop = element.scrollHeight; });
      await hudControls.mouse.wheel(0, 2000);
      assert.equal(await hudControls.evaluate(() => document.scrollingElement.scrollTop), 0);
      assert.deepEqual(await footer.boundingBox(), before);
      await hudControls.mouse.move(450, height - 15); await hudControls.mouse.wheel(0, 600);
      assert.equal(await hudControls.evaluate(() => document.scrollingElement.scrollTop), 0);
      assert.deepEqual(await footer.boundingBox(), before);
      await hudControls.getByRole('button', { name: '更多阅读操作', exact: true }).click();
      const menu = hudControls.getByRole('menu', { name: '更多阅读操作', exact: true }); await menu.waitFor();
      const menuBox = await menu.boundingBox(); assert.ok(menuBox && menuBox.y >= 0 && menuBox.y + menuBox.height <= height, JSON.stringify(menuBox));
      const content = await menu.evaluate(element => ({ height: element.clientHeight, scroll: element.scrollHeight, maxHeight: getComputedStyle(element).maxHeight }));
      if (height === 140) assert.ok(content.scroll > content.height, JSON.stringify(content));
      if (content.scroll > content.height) {
        // Finish the preceding boundary-wheel gesture before starting an independent menu gesture.
        // Chromium otherwise keeps its wheel latch on the reading pane, even after opening the menu.
        await hudControls.waitForTimeout(350);
        await hudControls.mouse.move(menuBox.x + 30, menuBox.y + 25); await hudControls.mouse.wheel(0, 500);
        await hudControls.waitForFunction(() => document.querySelector('[role="menu"]').scrollTop > 0, null, { timeout: 5000 }).catch(error => { throw new Error(`Menu wheel at ${height}px: ${error.message}`); });
      }
      const last = menu.getByRole('menuitem', { name: '断开游戏连接…', exact: true }); await last.scrollIntoViewIfNeeded();
      const lastBox = await last.boundingBox(); assert.ok(lastBox && lastBox.y >= 0 && lastBox.y + lastBox.height <= height);
      assert.equal(await hudControls.evaluate(() => document.scrollingElement.scrollTop), 0);
      assert.deepEqual(await footer.boundingBox(), before);
      assert.deepEqual(await hudControls.locator('html,body,#root,[data-ui-surface="hud"]').evaluateAll(nodes => nodes.map(node=>node.scrollTop)), [0,0,0,0], 'Only the declared reading/menu regions may scroll, including automatic focus scrolling.');
      await hudControls.screenshot({ path: path.join(out, `hud-internal-scroll-${height}.candidate.png`) });
      await hudControls.keyboard.press('Escape');
      await menu.waitFor({ state: 'hidden' });
    }
  });
  await check('UI-31-legacy-whitespace-gaps-preserve-reading-and-token-identity', async () => {
    const gaps = await pageFor('hud', 'reading');
    await gaps.evaluate(() => {
      const port = window.uiHarness; const source = ' \t雨\n駅\u3000🙂\r\n'; const encoder = new TextEncoder(); let cursor = 0;
      const tokens = ['雨', '駅', '🙂'].map((surface, index) => {
        const start = source.indexOf(surface, cursor); cursor = start + surface.length;
        return { surface, lemma: surface, reading: ['あめ', 'えき', ''][index], partOfSpeech: '名詞', known: true, utf16Start: start, utf16End: cursor, byteStart: encoder.encode(source.slice(0, start)).length, byteEnd: encoder.encode(source.slice(0, cursor)).length };
      });
      const unit = { ...port.state.current, sourceText: source, tokens };
      port.publish({ current: unit, history: { units: [unit], nextCursor: null } });
    });
    await gaps.getByRole('button', { name: '駅，读音 えき', exact: true }).waitFor();
    const original = await gaps.getByTestId('source-text').evaluate(element => { const copy = element.cloneNode(true); copy.querySelectorAll('rt').forEach(node => node.remove()); return copy.textContent; });
    assert.equal(original, ' \t雨\n駅\u3000🙂\r\n');
    await gaps.getByRole('button', { name: '駅，读音 えき', exact: true }).click();
    await gaps.getByRole('dialog', { name: '本地词典', exact: true }).getByRole('heading', { name: '駅', exact: true }).waitFor();
    await gaps.getByRole('button', { name: '关闭词卡', exact: true }).click();
    await gaps.evaluate(() => { const port = window.uiHarness; const unit = { ...port.state.current, tokens: port.state.current.tokens.map((token, index) => index === 1 ? { ...token, byteStart: token.byteStart + 1 } : token) }; port.publish({ current: unit }); });
    await gaps.waitForFunction(() => document.querySelector('[data-testid="source-text"]').querySelectorAll('button').length === 0);
    assert.equal(await gaps.getByTestId('source-text').textContent(), original);
    await gaps.evaluate(() => {
      const port = window.uiHarness; const unit = { ...port.state.current, sourceText: '雨猫駅', tokens: [{ surface: '雨', lemma: '雨', reading: 'あめ', partOfSpeech: '名詞', known: true, utf16Start: 0, utf16End: 1, byteStart: 0, byteEnd: 3 }, { surface: '駅', lemma: '駅', reading: 'えき', partOfSpeech: '名詞', known: true, utf16Start: 2, utf16End: 3, byteStart: 6, byteEnd: 9 }] };
      port.publish({ current: unit });
    });
    await gaps.getByTestId('source-text').filter({ hasText: '雨猫駅' }).waitFor();
    assert.equal(await gaps.getByTestId('source-text').getByRole('button').count(), 0, 'Missing non-whitespace text must not be silently treated as a valid token layout.');
    await gaps.close();
  });
  await check('UI-32-applied-connection-control-does-not-apply-drafts', async () => {
    const runtime = await pageFor('settings', 'configuration-lifecycle');
    await enterConfigurationSection(runtime,'service-advanced');
    await runtime.getByRole('textbox', { name: '连接名称', exact: true }).fill('UI unsaved rename');
    await runtime.evaluate(() => {
      const port = window.uiHarness; const configuration = port.state.configuration;
      port.runtimeResponses.push({ runtimeConnections: configuration.runtimeConnections.map(view => ({ ...view, suspended: true, controlRevision: view.controlRevision + 1 })), runtimePurposes: configuration.runtimePurposes.map(view => ({ ...view, blockedConnectionIds: ['ui-connection-a'], authorizedEngineIds: [] })) });
    });
    await enterConfigurationRuntime(runtime,'connection');
    await runtime.getByRole('button', { name: '立即停用连接', exact: true }).click();
    await runtime.getByRole('button', { name: '检查并恢复连接…', exact: true }).waitFor();
    await enterConfigurationSection(runtime,'service-advanced');
    assert.equal(await runtime.getByRole('textbox', { name: '连接名称', exact: true }).inputValue(), 'UI unsaved rename');
    await enterConfigurationRuntime(runtime,'connection');
    const stop = await runtime.evaluate(() => window.uiHarness.commands.at(-1).command);
    assert.deepEqual(stop, { kind: 'set_connection_suspended', payload: { connectionId: 'ui-connection-a', suspended: true, expectedControlRevision: 4 } });
    assert.equal(await runtime.evaluate(() => window.uiHarness.commands.length), 1);
    await runtime.getByRole('button', { name: '保存草稿', exact: true }).click(); await runtime.getByText('草稿已保存', { exact: true }).waitFor();
    await runtime.getByRole('button', { name: '测试当前对象', exact: true }).click();
    await runtime.getByText('UI transport does not call a real service', { exact: false }).waitFor();
    assert.equal(await runtime.evaluate(() => window.uiHarness.state.configuration.runtimeConnections[0].suspended), true);
    // A new runtime state has no approved same-state reference. Keep a review candidate without claiming visual acceptance.
    await runtime.locator('#root').screenshot({ path: path.join(out, 'connection-runtime-stopped.candidate.png') });
    assert.equal(await runtime.evaluate(() => window.uiHarness.state.configuration.applied.connections[0].displayName), 'UI applied shared service');
    await runtime.getByRole('button', { name: '检查并恢复连接…', exact: true }).click();
    const review = runtime.getByRole('dialog', { name: '恢复已应用连接？', exact: true });
    await review.getByText('UI applied shared service', { exact: false }).waitFor();
    assert.equal(await review.getByText('UI unsaved rename', { exact: false }).count(), 0);
    assert.ok((await review.innerText()).includes('实时翻译')); assert.ok((await review.innerText()).includes('句子精读')); assert.ok((await review.innerText()).includes('UI inherited game'));
    const beforeCancel = await runtime.evaluate(() => window.uiHarness.commands.length);
    await review.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await runtime.evaluate(() => window.uiHarness.commands.length), beforeCancel);
    await runtime.getByRole('button', { name: '检查并恢复连接…', exact: true }).click();
    await runtime.evaluate(() => { const port = window.uiHarness; port.runtimeResponses.push({ runtimeConnections: port.state.configuration.runtimeConnections.map(view => ({ ...view, suspended: false, controlRevision: view.controlRevision + 1 })), runtimePurposes: port.state.configuration.runtimePurposes.map(view => ({ ...view, blockedConnectionIds: [], authorizedEngineIds: port.state.configuration.applied.bindings.find(binding => binding.bindingId === view.bindingId).engines.map(engine => engine.engineId) })) }); });
    await review.getByRole('button', { name: '恢复此已应用连接', exact: true }).click(); await review.waitFor({ state: 'hidden' });
    assert.deepEqual(await runtime.evaluate(() => window.uiHarness.commands.at(-1).command), { kind: 'set_connection_suspended', payload: { connectionId: 'ui-connection-a', suspended: false, expectedControlRevision: 5 } });
    assert.equal(await runtime.evaluate(() => window.uiHarness.commands.filter(item => item.command.kind === 'apply_configuration').length), 0);
    await enterConfigurationSection(runtime,'service-advanced');
    await runtime.getByRole('button', { name: '添加服务…', exact: true }).click(); await runtime.getByRole('textbox', { name: '新服务名称', exact: true }).fill('UI unapplied connection'); await runtime.getByRole('button', { name: '创建服务草稿', exact: true }).click();
    await enterConfigurationRuntime(runtime,'connection');
    await runtime.getByText('这个连接尚未应用，没有可操作的运行许可。保存草稿或测试不会使它运行。', { exact: true }).waitFor();
    assert.equal(await runtime.getByRole('button', { name: '立即停用连接', exact: true }).count(), 0);
    await runtime.close();
  });
  await check('UI-33-purpose-runtime-gate-is-independent-of-draft-and-connection-gates', async () => {
    const runtime = await pageFor('settings', 'configuration-lifecycle'); await enterConfigurationSection(runtime,'translation');
    await runtime.getByText('实时翻译的草稿启用意图', { exact: true }).click();
    await runtime.evaluate(() => { const port = window.uiHarness; port.runtimeResponses.push({ runtimePurposes: port.state.configuration.runtimePurposes.map(view => view.purpose === 'translation' ? { ...view, runtimeEnabled: false, authorizationEpoch: view.authorizationEpoch + 1, authorizedEngineIds: [] } : view) }); });
    await enterConfigurationRuntime(runtime,'purpose');
    await runtime.getByRole('button', { name: '立即停用此用途', exact: true }).click();
    await runtime.getByRole('button', { name: '检查并恢复用途…', exact: true }).waitFor();
    await enterConfigurationSection(runtime,'translation');
    assert.equal(await runtime.getByRole('switch', { name: /^实时翻译的草稿启用意图/ }).isChecked(), false);
    await enterConfigurationRuntime(runtime,'purpose');
    assert.deepEqual(await runtime.evaluate(() => window.uiHarness.commands.at(-1).command), { kind: 'set_purpose_runtime_enabled', payload: { scopeKey: 'global', purpose: 'translation', enabled: false, expectedAuthorizationEpoch: 7 } });
    await runtime.getByRole('button', { name: '检查并恢复用途…', exact: true }).click();
    const review = runtime.getByRole('dialog', { name: '恢复已应用用途？', exact: true });
    await runtime.evaluate(() => { window.uiHarness.failNextCommand = true; });
    await review.getByRole('button', { name: '恢复此已应用用途', exact: true }).click();
    await review.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor();
    assert.equal(await runtime.evaluate(() => window.uiHarness.state.configuration.runtimePurposes[0].runtimeEnabled), false);
    await runtime.evaluate(() => { const port = window.uiHarness; port.runtimeResponses.push({ runtimePurposes: port.state.configuration.runtimePurposes.map(view => view.purpose === 'translation' ? { ...view, runtimeEnabled: true, authorizationEpoch: view.authorizationEpoch + 1, blockedConnectionIds: ['ui-connection-a'], authorizedEngineIds: [] } : view) }); });
    await review.getByRole('button', { name: '恢复此已应用用途', exact: true }).click(); await review.waitFor({ state: 'hidden' });
    await runtime.locator('details[data-configuration-section="testing"]').getByText('没有获准运行的引擎', { exact: true }).waitFor();
    await runtime.getByText('连接或凭据阻断：UI applied shared service。恢复用途不会解除这些阻断。', { exact: true }).waitFor();
    await selectValue(runtime.getByRole('combobox', { name: '作用范围', exact: true }), 'game:ui-term-game');
    await runtime.getByRole('button', { name: '查看全局运行许可', exact: true }).click();
    assert.equal(await runtime.getByRole('combobox', { name: '作用范围', exact: true }).getAttribute('data-select-value'), 'global');
    await runtime.evaluate(() => {
      const port = window.uiHarness; const configuration = structuredClone(port.state.configuration);
      configuration.applied.revisionId = 'ui-applied-disabled'; configuration.appliedHeadRevision++;
      configuration.applied.bindings[0].enabled = false;
      configuration.runtimePurposes = configuration.runtimePurposes.map(view => ({ ...view, configRevisionId: configuration.applied.revisionId, ...(view.purpose === 'translation' ? { enabledIntent: false, runtimeEnabled: false, authorizationEpoch: view.authorizationEpoch + 1 } : {}) }));
      configuration.runtimeConnections = configuration.runtimeConnections.map(view => ({ ...view, configRevisionId: configuration.applied.revisionId }));
      port.publish({ configuration });
    });
    await enterConfigurationSection(runtime,'translation');
    await runtime.getByText('实时翻译的草稿启用意图', { exact: true }).click();
    assert.equal(await runtime.getByRole('switch', { name: /^实时翻译的草稿启用意图/ }).isChecked(), true);
    await enterConfigurationRuntime(runtime,'purpose');
    assert.equal(await runtime.getByRole('button', { name: '检查并恢复用途…', exact: true }).isDisabled(), true);
    assert.equal(await runtime.evaluate(() => window.uiHarness.commands.filter(item => item.command.kind === 'save_configuration' || item.command.kind === 'apply_configuration').length), 0);
    await runtime.close();
  });
  await check('UI-34-credential-impact-cancel-and-failed-cleanup-retry-retains-operation', async () => {
    const runtime = await pageFor('settings', 'configuration-lifecycle');
    await runtime.setViewportSize({ width: 1000, height: 720 });
    await runtime.evaluate(() => { Object.assign(document.getElementById('root').style, { left: '0', top: '0', width: '100%', height: '100%' }); });
    await enterConfigurationSection(runtime,'service-advanced');
    await runtime.getByRole('textbox', { name: '连接名称', exact: true }).fill('UI credential draft retained');
    await runtime.getByRole('button', { name: '检查撤销影响…', exact: true }).click();
    const review = runtime.getByRole('dialog', { name: '撤销这个系统凭据？', exact: true });
    await review.getByText('UI shared ·••1234', { exact: true }).waitFor();
    assert.ok((await review.innerText()).includes('UI applied shared service')); assert.ok((await review.innerText()).includes('实时翻译')); assert.ok((await review.innerText()).includes('句子精读'));
    await runtime.locator('#root').screenshot({ path: path.join(out, 'credential-revocation-review.candidate.png') });
    assert.equal(await review.getByText('UI credential draft retained', { exact: false }).count(), 0);
    assert.equal(await runtime.evaluate(() => window.uiHarness.revocations.length), 0);
    await review.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await runtime.evaluate(() => window.uiHarness.revocations.length), 0);
    await runtime.getByRole('button', { name: '检查撤销影响…', exact: true }).click();
    await runtime.evaluate(() => {
      const port = window.uiHarness; const configuration = structuredClone(port.state.configuration);
      configuration.runtimeConnections = configuration.runtimeConnections.map(view => ({ ...view, suspended: true, reasonCode: 'credential_revoked', controlRevision: view.controlRevision + 1 }));
      configuration.runtimePurposes = configuration.runtimePurposes.map(view => ({ ...view, blockedConnectionIds: ['ui-connection-a'], authorizedEngineIds: [] }));
      port.revocationResponses.push({ patch: { configuration, credentials: port.state.credentials.map(item => ({ ...item, state: 'revoked', lifecycleRevision: item.lifecycleRevision + 1 })) }, error: 'UI transport: Vault cleanup failed after safety revocation' });
    });
    await review.getByRole('button', { name: '确认撤销凭据', exact: true }).click();
    await review.getByRole('alert').filter({ hasText: 'Vault cleanup failed' }).waitFor();
    const request = await runtime.evaluate(() => window.uiHarness.revocations[0]);
    assert.equal(request.credentialId, 'ui-credential-shared'); assert.equal(request.expectedLifecycleRevision, 2); assert.equal(request.expectedImpactHash, 'd'.repeat(64)); assert.ok(request.operationId);
    assert.equal(await runtime.evaluate(() => window.uiHarness.state.credentials[0].state), 'revoked');
    await runtime.evaluate(() => { window.uiHarness.revocationResponses.push({ patch: {}, error: null }); });
    await review.getByRole('button', { name: '重试本次撤销', exact: true }).click(); await review.waitFor({ state: 'hidden' });
    assert.deepEqual(await runtime.evaluate(() => window.uiHarness.revocations[1]), request);
    assert.equal(await runtime.getByRole('textbox', { name: '连接名称', exact: true }).inputValue(), 'UI credential draft retained');
    assert.equal(await runtime.evaluate(() => window.uiHarness.commands.length), 0);
    await runtime.close();
  });
  await check('UI-35-stale-runtime-confirmation-is-blocked-and-can-be-rechecked', async () => {
    const runtime = await pageFor('settings', 'configuration-lifecycle');
    await runtime.evaluate(() => { const port = window.uiHarness; port.publish({ configuration: { ...port.state.configuration, runtimeConnections: port.state.configuration.runtimeConnections.map(view => ({ ...view, suspended: true })) } }); });
    await enterConfigurationSection(runtime,'service-advanced');
    await enterConfigurationRuntime(runtime,'connection');
    await runtime.getByRole('button', { name: '检查并恢复连接…', exact: true }).click();
    const review = runtime.getByRole('dialog', { name: '恢复已应用连接？', exact: true }); await review.waitFor();
    await runtime.evaluate(() => { const port = window.uiHarness; port.publish({ configuration: { ...port.state.configuration, runtimeConnections: port.state.configuration.runtimeConnections.map(view => ({ ...view, controlRevision: view.controlRevision + 1 })) } }); });
    await review.getByText('已应用版本或运行许可已变化，请关闭后重新核对。', { exact: true }).waitFor();
    assert.equal(await review.getByRole('button', { name: '恢复此已应用连接', exact: true }).isDisabled(), true);
    assert.equal(await runtime.evaluate(() => window.uiHarness.commands.length), 0);
    await review.getByRole('button', { name: '取消', exact: true }).click();
    await runtime.getByRole('button', { name: '检查并恢复连接…', exact: true }).click();
    assert.equal(await review.getByRole('button', { name: '恢复此已应用连接', exact: true }).isEnabled(), true);
    await runtime.close();
  });
  if (errors.length) behavior.push({ id: 'UI-browser-console', status: 'failed', errors });
  await check('UI-36-terminal-session-state-precedes-saved-source-and-pause', async () => {
    const terminalHud = await pageFor('hud', 'reading'); const terminalMain = await pageFor('main', 'reading');
    await terminalMain.getByRole('button', { name: '当前会话', exact: true }).click();
    for (const [state, label] of [['stopped', '游戏取文已断开'], ['failed', '游戏接入失败'], ['crashed', '上次游戏取文已中断'], ['unknown-runtime-state', '游戏取文状态未知']]) {
      for (const page of [terminalHud, terminalMain]) {
        await page.evaluate(({ state }) => { const port = window.uiHarness; port.publish({ session: { ...port.state.session, state, paused: true, sources: [{ sourceId: 'ui-saved-source', hookName: 'UI saved hook', hookCode: '', encoding: 'UTF-16', sample: port.state.current.sourceText, selected: true }] } }); }, { state });
        await page.getByText(label, { exact: true }).first().waitFor();
        assert.equal(await page.getByText('正在接收正文', { exact: true }).count(), 0);
        assert.equal(await page.getByText('自动输入已暂停', { exact: true }).count(), 0);
        assert.equal(await page.locator(page===terminalHud?'[data-testid=source-text]:visible':'[aria-label="当前原文与译文"]:visible').count(), 1);
      }
      assert.equal(await terminalHud.getByRole('button', { name: '继续自动输入', exact: true }).isDisabled(), true);
      assert.equal(await terminalMain.getByRole('button', { name: '继续自动输入', exact: true }).isDisabled(), true);
      assert.equal(await terminalMain.getByRole('button', { name: '重新选择文本流…', exact: true }).isDisabled(), true);
      assert.equal(await terminalMain.getByRole('button', { name: '关闭自动翻译', exact: true }).isDisabled(), true);
    }
    for (const page of [terminalHud, terminalMain]) {
      await page.evaluate(() => { const port = window.uiHarness; port.publish({ session: { ...port.state.session, state: 'active', paused: true } }); });
      await page.getByText('自动输入已暂停', { exact: true }).first().waitFor();
      await page.evaluate(() => { const port = window.uiHarness; port.publish({ session: { ...port.state.session, state: 'opening', paused: true } }); });
      await page.getByText('正在接入游戏', { exact: true }).first().waitFor();
      assert.equal(await page.getByText('正在接收正文', { exact: true }).count(), 0);
    }
    assert.equal(await terminalHud.getByRole('button', { name: '继续自动输入', exact: true }).isDisabled(), true);
    await terminalHud.close(); await terminalMain.close();
  });
  await check('UI-37-history-queries-page-and-filter-before-projection-slice', async () => {
    const history = await pageFor('main', 'reading');
    await history.evaluate(() => {
      const port = window.uiHarness; const base = port.state.current;
      const game = (id, title) => ({ gameId: id, displayName: title, originalTitle: null, sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1, target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } });
      const units = Array.from({ length: 110 }, (_, n) => ({ ...base, unitId: `ui-history-${n}`, gameId: n < 55 ? 'ui-game-a' : 'ui-game-b', sessionId: n < 55 ? `ui-session-a-${n}` : `ui-session-b-${n % 2}`, sourceText: `${n < 55 ? '甲' : '乙'}游戏记录 ${n}`, tokens: [], createdAtMs: base.createdAtMs - n * 1000 }));
      units.push({ ...base, unitId: 'ui-manual-beyond-first-page', gameId: null, sessionId: 'ui-manual-session', sourceText: '最早的手动输入', tokens: [], createdAtMs: base.createdAtMs - 200000 });
      units.push({ ...base, unitId: 'ui-gameless-hook', gameId: null, sessionId: 'ui-gameless-hook-session', sourceText: '无库关联的Hook原文', tokens: [], createdAtMs: base.createdAtMs - 210000 });
      for (const unit of units) unit.sourceSummary = { sessionId: unit.sessionId, gameId: unit.gameId, sourceKind: unit.unitId === 'ui-manual-beyond-first-page' ? 'manual' : 'hook', state: 'stopped', startedAtMs: unit.createdAtMs, endedAtMs: unit.createdAtMs };
      port.publish({ games: [game('ui-game-a', '甲游戏'), game('ui-game-b', '乙游戏')], history: { units, nextCursor: null }, current: units[0], session: { ...port.state.session, sessionId: units[0].sessionId, gameId: units[0].gameId } });
    });
    await history.getByRole('button', { name: '历史记录', exact: true }).click();
    const list = history.getByRole('complementary', { name: '历史筛选与记录', exact: true });
    await list.getByRole('button', { name: '甲游戏记录 0', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: /^甲游戏记录 / }).count(), 50);
    assert.equal(await list.getByRole('button', { name: /^乙游戏记录 / }).count(), 0);
    assert.equal(await list.getByRole('button', { name: '最早的手动输入', exact: false }).count(), 0);
    await list.getByRole('button', { name: '加载更早记录', exact: true }).click();
    await list.getByRole('button', { name: '乙游戏记录 99', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: /^甲游戏记录 |^乙游戏记录 / }).count(), 100);
    await list.getByRole('button', { name: '加载更早记录', exact: true }).click();
    const oldManual = list.getByRole('button', { name: '最早的手动输入', exact: false });
    await oldManual.waitFor();
    assert.ok((await oldManual.locator('small').innerText()).includes('手动输入'));
    assert.equal((await selectOptions(history.getByRole('combobox', { name: '会话筛选', exact: true }))).filter(option => option.value === 'ui-manual-session').length, 0);
    const gameless = list.getByRole('button', { name: '无库关联的Hook原文', exact: false });
    assert.ok((await gameless.locator('small').innerText()).includes('游戏原文'));
    assert.equal((await gameless.locator('small').innerText()).includes('手动输入'), false);
    await oldManual.click();
    await history.locator('[data-testid=source-text]:visible').filter({ hasText: '最早的手动输入' }).waitFor();
    const metadata = await history.locator('dl:visible').innerText();
    assert.ok(metadata.includes('手动输入')); assert.ok(metadata.includes('阅读会话')); assert.ok(metadata.includes('已结束'));
    await selectValue(history.getByRole('combobox', { name: '游戏筛选', exact: true }), 'manual');
    await list.getByRole('button', { name: '最早的手动输入', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: /^甲游戏记录 |^乙游戏记录 / }).count(), 0);
    assert.equal(await history.evaluate(() => window.uiHarness.historyQueries.at(-1).filter.sourceKind), 'manual');
    await selectValue(history.getByRole('combobox', { name: '游戏筛选', exact: true }), 'ui-game-a');
    await list.getByRole('button', { name: '甲游戏记录 0', exact: false }).waitFor();
    const sessions = history.getByRole('combobox', { name: '会话筛选', exact: true });
    assert.equal(await selectOptionCount(sessions, 51), 51);
    await history.getByRole('button', { name: '加载更早会话', exact: true }).click();
    assert.equal(await selectOptionCount(sessions, 56), 56);
    await selectValue(sessions, 'ui-session-a-54');
    await list.getByRole('button', { name: '甲游戏记录 54', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: /^甲游戏记录 / }).count(), 1);
    await selectValue(history.getByRole('combobox', { name: '游戏筛选', exact: true }), 'ui-game-b');
    await list.getByRole('button', { name: '乙游戏记录 55', exact: false }).waitFor();
    await selectValue(sessions, 'ui-session-b-1');
    await list.getByRole('button', { name: '乙游戏记录 109', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: /^乙游戏记录 / }).count(), 28);
    const scopeError = await history.evaluate(async () => { const port = window.uiHarness; const first = await port.historyPage({ filter: { gameId: 'ui-game-a', sourceKind: null, sessionId: null }, cursor: null, limit: 1 }); try { await port.historyPage({ filter: { gameId: 'ui-game-b', sourceKind: null, sessionId: null }, cursor: first.nextCursor, limit: 1 }); return false; } catch { return true; } });
    assert.equal(scopeError, true);
    await history.evaluate(() => { const port = window.uiHarness; port.historyHolds.push({ kind: 'units', gameId: 'ui-game-a', key: 'late-a-units' }, { kind: 'sessions', gameId: 'ui-game-a', key: 'late-a-sessions' }); });
    await selectValue(history.getByRole('combobox', { name: '游戏筛选', exact: true }), 'ui-game-a');
    await history.waitForFunction(() => window.uiHarness.historyPending.has('late-a-units') && window.uiHarness.historyPending.has('late-a-sessions'));
    await selectValue(history.getByRole('combobox', { name: '游戏筛选', exact: true }), 'ui-game-b');
    await list.getByRole('button', { name: '乙游戏记录 55', exact: false }).waitFor();
    await history.evaluate(() => { window.uiHarness.releaseHistory('late-a-units'); window.uiHarness.releaseHistory('late-a-sessions'); });
    await history.waitForFunction(() => window.uiHarness.historyReleased.includes('late-a-units') && window.uiHarness.historyReleased.includes('late-a-sessions'));
    await history.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assert.equal(await history.getByRole('combobox', { name: '游戏筛选', exact: true }).getAttribute('data-select-value'), 'ui-game-b');
    assert.equal(await list.getByRole('button', { name: /^甲游戏记录 / }).count(), 0);
    assert.equal(await list.getByRole('button', { name: /^乙游戏记录 / }).count(), 50);
    assert.equal((await selectOptions(sessions)).filter(option => option.value.startsWith('ui-session-a-')).length, 0);
    await history.evaluate(() => { const port = window.uiHarness; const unit = port.state.history.units.find(unit => unit.unitId === 'ui-gameless-hook'); port.publish({ current: unit, session: { ...port.state.session, sessionId: unit.sessionId, gameId: null, sourceKind: 'hook' } }); });
    await history.getByRole('button', { name: '当前会话', exact: true }).click();
    await history.locator('[aria-label="当前原文与译文"]:visible').filter({ hasText: '无库关联的Hook原文' }).waitFor();
    assert.equal(await history.locator('[aria-label="当前原文与译文"] footer span:visible').filter({ hasText: /^游戏原文 · / }).count(), 1);
    assert.equal(await history.locator('[aria-label="当前原文与译文"] footer span:visible').filter({ hasText: /^手动输入 · / }).count(), 0);
    await history.close();
  });
  await check('UI-38-history-revisions-current-location-and-unsaved-correction', async () => {
    const history = await pageFor('main', 'reading');
    await history.evaluate(() => {
      const port = window.uiHarness; const base = port.state.current; const privateId = `unit-${'b'.repeat(64)}`;
      const fill = Array.from({ length: 55 }, (_, n) => ({ ...base, unitId: `ui-fill-${n}`, sessionId: 'ui-fill-session', sourceText: `分页记录 ${n}`, tokens: [], createdAtMs: base.createdAtMs - n * 1000 }));
      const old = { ...base, unitId: privateId, sessionId: `session-${'c'.repeat(64)}`, sourceHash: 'd'.repeat(64), textRevision: 1, sourceText: '同一原句的旧版本', tokens: [], createdAtMs: base.createdAtMs - 60000, translations: [{ translationId: 'ui-translation-v1', taskId: 'ui-task-v1', text: '旧译文', providerLabel: 'UI revision engine', targetLanguage: 'zh-CN', correction: null }] };
      old.sourceSummary = { sessionId: old.sessionId, gameId: null, sourceKind: 'hook', state: 'active', startedAtMs: old.createdAtMs, endedAtMs: null };
      const current = { ...old, textRevision: 2, sourceText: '同一原句的新版本', translations: [{ ...old.translations[0], translationId: 'ui-translation-v2', taskId: 'ui-task-v2', text: '新译文' }] };
      port.publish({ current, history: { units: [...fill, old, current], nextCursor: null }, session: { ...port.state.session, sessionId: current.sessionId } });
    });
    await history.getByRole('button', { name: '历史记录', exact: true }).click();
    const list = history.getByRole('complementary', { name: '历史筛选与记录', exact: true });
    await list.getByRole('button', { name: '分页记录 0', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: '同一原句的新版本', exact: false }).count(), 0);
    await history.getByRole('button', { name: '定位当前句', exact: true }).click();
    await history.locator('[data-testid=source-text]:visible').filter({ hasText: '同一原句的新版本' }).waitFor();
    await list.getByRole('button', { name: '加载更早记录', exact: true }).click();
    await list.getByRole('button', { name: '同一原句的旧版本', exact: false }).waitFor();
    assert.equal(await list.getByRole('button', { name: '同一原句的新版本', exact: false }).count(), 1);
    await list.getByRole('button', { name: '同一原句的旧版本', exact: false }).click();
    await history.locator('[data-testid=source-text]:visible').filter({ hasText: '同一原句的旧版本' }).waitFor();
    await history.locator('p:visible').filter({ hasText: /^旧译文$/ }).waitFor();
    await list.getByRole('button', { name: '同一原句的新版本', exact: false }).click();
    await history.locator('[data-testid=source-text]:visible').filter({ hasText: '同一原句的新版本' }).waitFor();
    await history.locator('p:visible').filter({ hasText: /^新译文$/ }).waitFor();
    const visible = await history.locator('body').innerText();
    for (const internal of [`unit-${'b'.repeat(64)}`, `session-${'c'.repeat(64)}`, 'd'.repeat(64)]) assert.equal(visible.includes(internal), false);
    await history.getByRole('button', { name: '修正 UI revision engine', exact: true }).click();
    await history.getByRole('textbox', { name: '人工修正', exact: true }).fill('人工修正保存失败后保留');
    await history.evaluate(() => { window.uiHarness.failNextCommand = true; });
    await history.getByRole('button', { name: '保存修正', exact: true }).click();
    await history.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor();
    assert.equal(await history.getByRole('textbox', { name: '人工修正', exact: true }).inputValue(), '人工修正保存失败后保留');
    assert.equal(await history.getByRole('combobox', { name: '游戏筛选', exact: true }).isDisabled(), true);
    assert.equal(await history.getByRole('combobox', { name: '会话筛选', exact: true }).isDisabled(), true);
    assert.equal(await list.getByRole('button', { name: '同一原句的旧版本', exact: false }).isDisabled(), true);
    assert.equal(await history.getByRole('button', { name: '定位当前句', exact: true }).isDisabled(), true);
    await history.getByRole('button', { name: '手动翻译', exact: true }).click();
    await history.getByRole('button', { name: '历史记录', exact: true }).click();
    assert.equal(await history.getByRole('textbox', { name: '人工修正', exact: true }).inputValue(), '人工修正保存失败后保留');
    assert.equal(await history.evaluate(() => window.uiHarness.state.current.translations[0].correction), null);
    const command = await history.evaluate(() => window.uiHarness.commands.at(-1).command);
    assert.equal(command.kind, 'save_correction'); assert.equal(command.payload.translationId, 'ui-translation-v2'); assert.equal(command.payload.baseRevision, 0);
    await history.close();
  });
  await check('UI-39-study-tabs-fixed-object-and-explicit-model-actions', async () => {
    const study = await pageFor('learning', 'learning');
    const dictionary = study.getByRole('tab', { name: '内置词典', exact: true });
    const grammar = study.getByRole('tab', { name: '本句精读', exact: true });
    assert.equal(await dictionary.getAttribute('aria-selected'), 'true');
    await study.getByRole('heading', { name: '放課後', exact: true }).waitFor();
    const identity = await study.evaluate(() => window.uiHarness.state.learningObject);
    assert.equal(await study.getByText('游戏原文', { exact: true }).count(), 1);
    assert.equal(await study.getByText('手动输入', { exact: true }).count(), 0);
    await grammar.click();
    await study.getByRole('heading', { name: '为这类理解选择一个模型', exact: true }).waitFor();
    assert.equal(await study.getByTestId('source-text').count(), 1);
    await grammar.press('ArrowRight');
    assert.equal(await dictionary.getAttribute('aria-selected'), 'true');
    assert.equal(await dictionary.evaluate(element => element === document.activeElement), true);
    assert.deepEqual(await study.evaluate(() => window.uiHarness.state.learningObject), identity);
    assert.equal(await study.evaluate(() => window.uiHarness.studies.length), 0);
    await study.getByRole('button', { name: '解释选词', exact: true }).click();
    await study.waitForFunction(() => window.uiHarness.studies.length === 1);
    assert.equal(await grammar.getAttribute('aria-selected'), 'true');
    assert.deepEqual(await study.evaluate(() => window.uiHarness.studies[0]), { unitId: identity.unit.unitId, textRevision: identity.unit.textRevision, purpose: 'context_word', tokenIndex: identity.tokenIndex });
    assert.deepEqual(await study.evaluate(() => window.uiHarness.state.learningObject), identity);
    await study.evaluate(() => { const port = window.uiHarness; const unit = { ...port.state.learningObject.unit, sourceSummary: { ...port.state.learningObject.unit.sourceSummary, sourceKind: 'manual', state: 'stopped' } }; port.publish({ learningObject: { unit, tokenIndex: null } }); });
    await study.getByText('手动输入', { exact: true }).waitFor();
    assert.equal(await study.getByText('游戏原文', { exact: true }).count(), 0);
    await study.close();
  });

  await check('UI-40-reading-logical-font-sizes-and-sakura-terminal-punctuation', async () => {
    const logical = await pageFor('hud', 'reading', { width: 560, height: 172 }); await publishDesignReading(logical);
    const source = logical.getByTestId('source-text');
    const translation = logical.getByRole('article').getByText(designGame.lines[0].translation, { exact: true });
    assert.equal(await source.evaluate(element => getComputedStyle(element).fontSize), '18px');
    assert.equal(await translation.evaluate(element => getComputedStyle(element).fontSize), '24px');
    const punctuation = await source.evaluate(element => {
      const tokens = [...element.querySelectorAll('[data-token-index]')]; const last = tokens.at(-1); const preceding = tokens.at(-2);
      const rect = node => { const range = document.createRange(); range.selectNodeContents(node); return [...range.getClientRects()].filter(box => box.width > 0).at(-1).toJSON(); };
      return { last: rect(last), preceding: rect(preceding.querySelector('ruby').firstChild), punctuationTag: last.tagName, text: last.textContent };
    });
    assert.equal(punctuation.text, '。'); assert.notEqual(punctuation.punctuationTag, 'BUTTON');
    assert.ok(Math.abs(punctuation.last.y - punctuation.preceding.y) < 2, `Default Sakura terminal punctuation detached: ${JSON.stringify(punctuation)}`);
    await logical.evaluate(() => { const port = window.uiHarness; port.publish({ preferences: { ...port.state.preferences, sourceFontPt: 36, translationFontPt: 48 } }); });
    await logical.waitForFunction(() => getComputedStyle(document.querySelector('[data-testid=source-text]')).fontSize === '36px');
    assert.equal(await source.evaluate(element => getComputedStyle(element).fontSize), '36px');
    assert.equal(await translation.evaluate(element => getComputedStyle(element).fontSize), '48px');
    await logical.setViewportSize({ width: 390, height: 290 });
    await logical.evaluate(() => {
      const port = window.uiHarness; let byte = 0; let utf16 = 0;
      const parts = [['「', '', '補助記号'], ['学校', 'がっこう', '名詞'], ['。', '', '補助記号'], ['」', '', '補助記号'], ['\n', '', '空白'], ['次', '', '名詞'], ['の', '', '助詞'], ['行', 'ぎょう', '名詞'], ['。', '', '補助記号']];
      const tokens = parts.map(([surface, reading, partOfSpeech]) => { const byteStart = byte; const utf16Start = utf16; byte += new TextEncoder().encode(surface).length; utf16 += surface.length; return { surface, lemma: surface, reading, partOfSpeech, byteStart, byteEnd: byte, utf16Start, utf16End: utf16, known: true }; });
      const unit = { ...port.state.current, unitId: 'ui-brackets-and-linebreak', sourceText: parts.map(part => part[0]).join(''), tokens, translations: [] };
      port.publish({ current: unit, history: { units: [unit], nextCursor: null } });
    });
    await logical.getByRole('button', { name: '学校，读音 がっこう', exact: true }).waitFor();
    const brackets = await source.evaluate(element => {
      const rect = node => { const range = document.createRange(); range.selectNodeContents(node); return [...range.getClientRects()].filter(box => box.width > 0).at(-1).toJSON(); };
      const token = index => element.querySelector(`[data-token-index="${index}"]`);
      const copy = element.cloneNode(true); copy.querySelectorAll('rt').forEach(reading => reading.remove());
      return { text: copy.textContent, open: rect(token(0)), base: rect(token(1).querySelector('ruby').firstChild), period: rect(token(2)), close: rect(token(3)), next: rect(token(5).querySelector('ruby').firstChild), lookupIndices: [...element.querySelectorAll('button[data-token-index]')].map(button => Number(button.dataset.tokenIndex)) };
    });
    assert.equal(brackets.text, '「学校。」\n次の行。');
    for (const mark of [brackets.open, brackets.period, brackets.close]) assert.ok(Math.abs(mark.y - brackets.base.y) < 2, `Bracket group detached: ${JSON.stringify(brackets)}`);
    assert.ok(brackets.next.y > brackets.close.y, `Original newline was lost: ${JSON.stringify(brackets)}`);
    assert.deepEqual(brackets.lookupIndices, [1, 5, 6, 7]);
    await logical.getByRole('button', { name: '学校，读音 がっこう', exact: true }).click();
    await logical.getByRole('dialog', { name: '本地词典', exact: true }).getByRole('button', { name: '打开学习窗口', exact: true }).click();
    await logical.waitForFunction(() => window.uiHarness.state.learningObject?.unit.unitId === 'ui-brackets-and-linebreak');
    assert.equal(await logical.evaluate(() => window.uiHarness.state.learningObject.tokenIndex), 1);
    assert.equal(await logical.evaluate(() => window.uiHarness.state.learningObject.unit.tokens[1].surface), '学校');
    await logical.close();
  });
  await check('UI-41-model-default-templates-draft-restore-and-copy-cas', async () => {
    const config = await pageFor('settings', 'model-config');
    await enterConfigurationSection(config,'service-advanced');
    const section = config.getByRole('region', { name: '实时翻译模型默认模板', exact: true });
    const selection = config.getByRole('combobox', { name: '实时翻译默认模板', exact: true });
    assert.equal(await selection.getAttribute('data-select-value'), '');
    await section.getByRole('button', { name: '新建此模型专用模板', exact: true }).click();
    await config.getByRole('textbox', { name: '实时翻译专用模板名称', exact: true }).fill('UI 独立翻译模板');
    await config.getByRole('textbox', { name: '实时翻译专用user模板', exact: true }).fill('请翻译：{{source}}');
    const selectedId = await selection.getAttribute('data-select-value'); assert.ok(selectedId);
    assert.equal(await config.evaluate(() => window.uiHarness.commands.length), 0);
    await enterConfigurationSection(config,'service-advanced');
    assert.equal(await config.getByRole('button', { name: '复制模型配置…', exact: true }).isEnabled(), false);
    await enterConfigurationSection(config,'service-advanced');
    await config.getByRole('button', { name: '保存草稿', exact: true }).click();
    await config.getByText('草稿已保存', { exact: true }).waitFor();
    const saved = await config.evaluate(() => window.uiHarness.commands.at(-1).command);
    assert.equal(saved.kind, 'save_configuration');
    assert.deepEqual(saved.payload.configuration.models[0].defaultTemplates, { translation: selectedId });
    assert.equal(saved.payload.configuration.templates.find(template => template.templateId === selectedId).compatibility.modelConfigurationId, 'ui-model-a');
    assert.equal(await config.evaluate(() => window.uiHarness.state.configuration.applied), null);
    await chooseConfigurationService(config, 'ui-model-b');
    assert.equal(await config.getByRole('option', { name: 'UI 独立翻译模板 · 此模型专用', exact: true }).count(), 0);
    await chooseConfigurationService(config, 'ui-model-a');
    assert.equal(await selection.getAttribute('data-select-value'), selectedId);
    await section.getByRole('button', { name: '恢复内置用途模板', exact: true }).click();
    assert.equal(await selection.getAttribute('data-select-value'), '');
    assert.deepEqual(await config.evaluate(() => window.uiHarness.state.configuration.draft.models[0].defaultTemplates), { translation: selectedId });
    await config.getByRole('button', { name: '保存草稿', exact: true }).click();
    await config.getByText('草稿已保存', { exact: true }).waitFor();
    assert.deepEqual(await config.evaluate(() => window.uiHarness.commands.at(-1).command.payload.configuration.models[0].defaultTemplates), {});
    await enterConfigurationSection(config,'service-advanced');
    await config.getByRole('button', { name: '复制模型配置…', exact: true }).click();
    const dialog = config.getByRole('dialog', { name: '复制已保存的模型配置？', exact: true });
    await dialog.getByRole('textbox', { name: '模型副本名称', exact: true }).fill('UI isolated copy');
    const before = await config.evaluate(() => window.uiHarness.commands.length);
    await config.evaluate(() => { const port = window.uiHarness; port.publish({ configuration: { ...port.state.configuration, draftHeadRevision: port.state.configuration.draftHeadRevision + 1 } }); });
    await dialog.getByText('已保存草稿已变化，请取消后重新核对复制来源。', { exact: true }).waitFor();
    assert.equal(await dialog.getByRole('button', { name: '创建独立副本', exact: true }).isEnabled(), false);
    assert.equal(await config.evaluate(() => window.uiHarness.commands.length), before);
    await dialog.getByRole('button', { name: '取消', exact: true }).click();
    await config.getByRole('button', { name: '复制模型配置…', exact: true }).click();
    await dialog.getByRole('textbox', { name: '模型副本名称', exact: true }).fill('UI isolated copy');
    const head = await config.evaluate(() => window.uiHarness.state.configuration.draftHeadRevision);
    // A prepared server projection checks command/receipt UI wiring only. Copy semantics are
    // independently exercised against the production Rust repository, never implemented here.
    await config.evaluate(() => {
      const port = window.uiHarness; const configuration = structuredClone(port.state.configuration);
      configuration.draft.models.push({ ...structuredClone(configuration.draft.models[0]), modelId: 'ui-prepared-copy', displayName: 'UI isolated copy' });
      configuration.draft.revisionId = 'ui-copy-server-revision'; configuration.draftHeadRevision++;
      port.copyResponses.push({ configuration, resultId: 'ui-prepared-copy' });
    });
    await dialog.getByRole('button', { name: '创建独立副本', exact: true }).click();
    await dialog.waitFor({ state: 'hidden' });
    assert.deepEqual(await config.evaluate(() => window.uiHarness.commands.at(-1).command), { kind: 'copy_model_configuration', payload: { modelId: 'ui-model-a', newDisplayName: 'UI isolated copy', expectedHeadRevision: head } });
    assert.equal(await config.locator('[data-service-model="ui-prepared-copy"]').getAttribute('aria-pressed'), 'true');
    assert.equal(await config.evaluate(() => window.uiHarness.state.configuration.applied), null);
    assert.equal(await config.evaluate(() => window.uiHarness.commands.some(item => item.command.kind === 'apply_configuration')), false);
    assert.equal(await config.evaluate(() => window.uiHarness.studies.length), 0);
    await config.close();
  });
  await check('UI-42-word-card-independent-freeze-explicit-action-and-generation', async () => {
    // This controlled host checks component/port semantics only. Native monitor
    // placement, focus and cross-window bounds require the packaged-app run.
    const hud = await pageFor('hud', 'reading', { width: 660, height: 700 });
    await hud.evaluate(() => { const port = window.uiHarness; port.publish({ learningObject: { unit: structuredClone(port.state.current), tokenIndex: 3 } }); });
    const learningBefore = await hud.evaluate(() => structuredClone(window.uiHarness.state.learningObject));
    await hud.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).click();
    const card = hud.getByRole('dialog', { name: '本地词典', exact: true });
    await card.getByRole('heading', { name: '放課後', exact: true }).waitFor();
    const frozen = await hud.evaluate(() => structuredClone(window.uiHarness.wordCard));
    const request = await hud.evaluate(() => window.uiHarness.wordCardRequests.at(-1));
    assert.deepEqual(Object.keys(request).sort(), ['anchorRect', 'textRevision', 'tokenIndex', 'unitId']);
    assert.equal(request.unitId, frozen.unit.unitId); assert.equal(request.tokenIndex, 0);
    for (const value of Object.values(request.anchorRect)) assert.ok(Number.isFinite(value));
    assert.ok(request.anchorRect.width > 0 && request.anchorRect.height > 0);
    assert.deepEqual(await hud.evaluate(() => window.uiHarness.state.learningObject), learningBefore);
    assert.equal(await hud.evaluate(() => window.uiHarness.studies.length), 0);
    await hud.evaluate(() => {
      const port = window.uiHarness; const next = { ...structuredClone(port.state.current), unitId: 'ui-next-with-same-word', createdAtMs: port.state.current.createdAtMs + 1000 };
      port.publish({ current: next, history: { units: [next, ...port.state.history.units], nextCursor: null } });
    });
    assert.deepEqual(await hud.evaluate(() => window.uiHarness.wordCard), frozen);
    assert.deepEqual(await hud.evaluate(() => window.uiHarness.state.learningObject), learningBefore);
    await card.getByRole('button', { name: '关闭词卡', exact: true }).click();
    await card.waitFor({ state: 'hidden' });
    assert.deepEqual(await hud.evaluate(() => window.uiHarness.state.learningObject), learningBefore);
    await hud.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).click();
    await card.getByRole('heading', { name: '放課後', exact: true }).waitFor();
    const second = await hud.evaluate(() => window.uiHarness.wordCard.cardId); assert.notEqual(second, frozen.cardId);
    await hud.evaluate(id => window.uiHarness.closeWordCard(id), frozen.cardId);
    assert.equal(await hud.evaluate(() => window.uiHarness.wordCard.cardId), second);
    await hud.keyboard.press('Escape'); await card.waitFor({ state: 'hidden' });
    await hud.getByRole('button', { name: '放課後，读音 ほうかご', exact: true }).click();
    await card.getByRole('button', { name: 'AI 语境解释', exact: true }).click();
    await card.waitFor({ state: 'hidden' });
    assert.deepEqual(await hud.evaluate(() => window.uiHarness.studies), [{ unitId: 'ui-next-with-same-word', textRevision: 1, purpose: 'context_word', tokenIndex: 0 }]);
    assert.equal(await hud.evaluate(() => window.uiHarness.state.learningObject.unit.unitId), 'ui-next-with-same-word');
    assert.equal(await hud.evaluate(() => window.uiHarness.state.learningObject.tokenIndex), 0);
    await hud.close();
  });
  await check('UI-44-game-arguments-preserve-order-empty-values-and-cancel', async () => {
    const game = await pageFor();
    await game.getByRole('button', { name: '添加游戏…', exact: true }).first().click();
    await game.getByRole('button', { name: '选择文件…', exact: true }).click();
    await selectValue(game.getByRole('combobox', { name: '原文语言', exact: true }), 'ja');
    const argumentsBefore = ['--mode', 'value with spaces and "quotes"', '', '日本語・引数\\path'];
    for (const [index, value] of argumentsBefore.entries()) {
      await game.getByRole('button', { name: '添加启动参数', exact: true }).click();
      await game.getByRole('textbox', { name: `启动参数 ${index + 1}`, exact: true }).fill(value);
    }
    await game.getByRole('button', { name: '添加到游戏库', exact: true }).click();
    await game.getByRole('dialog').waitFor({ state: 'hidden' });
    assert.deepEqual(await game.evaluate(() => window.uiHarness.state.games[0].target.arguments), argumentsBefore);
    const savedCommands = await game.evaluate(() => window.uiHarness.commands.length);
    await game.getByRole('button', { name: '编辑…', exact: true }).click();
    for (const [index, value] of argumentsBefore.entries()) assert.equal(await game.getByRole('textbox', { name: `启动参数 ${index + 1}`, exact: true }).inputValue(), value);
    await game.getByRole('button', { name: '移除启动参数 2', exact: true }).click();
    await game.getByRole('button', { name: '取消', exact: true }).click();
    assert.deepEqual(await game.evaluate(() => window.uiHarness.state.games[0].target.arguments), argumentsBefore);
    assert.equal(await game.evaluate(() => window.uiHarness.commands.length), savedCommands);
    await game.getByRole('button', { name: '编辑…', exact: true }).click();
    await game.getByRole('button', { name: '移除启动参数 2', exact: true }).click();
    await game.getByRole('button', { name: '保存到游戏库', exact: true }).click();
    await game.getByRole('dialog').waitFor({ state: 'hidden' });
    assert.deepEqual(await game.evaluate(() => window.uiHarness.state.games[0].target.arguments), [argumentsBefore[0], argumentsBefore[2], argumentsBefore[3]]);
    assert.equal(await game.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    await game.close();
  });
  await check('UI-45-settings-ten-categories-search-and-real-connection-reuse', async () => {
    const settings = await pageFor('settings', 'model-config');
    const nav = settings.getByRole('navigation', { name: '设置分类', exact: true });
    const labels = ['通用', '输入与HOOK', 'OCR引擎', '翻译服务', '文本处理', '语音', '显示与快捷键', '网络服务', '配置与数据', '更新与诊断'];
    assert.deepEqual(await nav.getByRole('button').allTextContents(), labels);
    await enterConfigurationSection(settings,'service-advanced');
    await settings.getByRole('textbox', { name: '连接名称', exact: true }).fill('UI shared unsaved connection');
    await nav.getByRole('button', { name: '网络服务', exact: true }).click();
    await settings.getByRole('button', { name: '前往连接设置', exact: true }).click();
    assert.equal(await settings.getByRole('textbox', { name: '连接名称', exact: true }).inputValue(), 'UI shared unsaved connection');
    assert.equal(await settings.evaluate(() => window.uiHarness.commands.length), 0);
    await nav.getByRole('button', { name: 'OCR引擎', exact: true }).click();
    await settings.getByRole('heading', { name: 'OCR引擎', exact: true }).waitFor();
    assert.equal(await settings.getByRole('button', { name: /启动OCR|开启OCR|开始捕获/ }).count(), 0);
    await nav.getByRole('button', { name: '语音', exact: true }).click();
    assert.equal(await settings.getByRole('button', { name: /朗读|启动语音/ }).count(), 0);
    await settings.getByRole('textbox', { name: '搜索设置', exact: true }).fill('网络');
    assert.deepEqual(await nav.getByRole('button').allTextContents(), ['网络服务']);
    await settings.getByRole('textbox', { name: '搜索设置', exact: true }).fill('');
    assert.equal(await nav.getByRole('button').count(), 10);
    await nav.getByRole('button', { name: '翻译服务', exact: true }).click();
    assert.equal(await settings.getByRole('textbox', { name: '连接名称', exact: true }).inputValue(), 'UI shared unsaved connection');
    assert.equal(await settings.evaluate(() => window.uiHarness.commands.length), 0);
    await settings.close();
  });
  await check('UI-46-target-list-and-mapping-stay-together-with-real-identity', async () => {
    const main = await pageFor();
    await main.evaluate(() => {
      const candidate = { ...window.uiHarness.targetCandidates[0], windowHandle: '424242', macosHostPid: null, macosWindowId: null };
      window.uiHarness.targetCandidates = [candidate, { ...candidate, candidateId: 'ui-no-window', windowTitle: 'UI process without window', hasWindow: false }];
    });
    await main.getByRole('button', { name: '连接运行中的游戏…', exact: true }).first().click();
    await main.getByRole('button', { name: '扫描进程', exact: true }).click();
    assert.equal(await main.getByRole('radio').count(), 1);
    await main.getByRole('button', { name: '进程列表', exact: true }).click();
    assert.equal(await main.getByRole('radio').count(), 2);
    await main.getByRole('radio').first().check();
    const mapping = main.getByRole('region', { name: '关联与实际取文目标', exact: true });
    await mapping.waitFor();
    assert.equal(await main.getByRole('button', { name: '核对目标', exact: true }).count(), 0);
    assert.equal(await main.getByRole('radiogroup', { name: '实际取文目标', exact: true }).isVisible(), true);
    assert.equal(await mapping.isVisible(), true);
    await mapping.getByText('高级运行身份', { exact: true }).click();
    assert.equal(await mapping.getByText('424242', { exact: true }).count(), 1);
    assert.equal(await mapping.getByText('未提供', { exact: true }).count(), 2);
    assert.equal(await main.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    await main.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await main.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    await main.getByRole('button', { name: '连接运行中的游戏…', exact: true }).first().click();
    await main.getByRole('radio').first().waitFor();
    assert.equal(await main.getByRole('radio', { checked: true }).count(), 0);
    assert.equal(await main.getByRole('region', { name: '关联与实际取文目标', exact: true }).count(), 0);
    await main.close();
  });
  await check('UI-47-batch-switch-cancel-and-failed-atomic-intent-retain-drafts', async () => {
    const main = await pageFor();
    await main.evaluate(() => { window.uiHarness.executableSelections = ['A', 'B'].map(name => ({ path: `/UI-test/${name}.exe`, displayName: `UI batch ${name}`, architecture: 'x86', environmentKey: 'crossover:/UI-test/Bottle', bottleLocator: '/UI-test/Bottle' })); });
    const open = async () => { await main.getByRole('button', { name: '添加游戏…', exact: true }).first().click(); await main.getByRole('button', { name: '选择文件…', exact: true }).click(); };
    await open();
    await selectValue(main.getByRole('combobox', { name: '原文语言', exact: true }), 'ja');
    await main.getByRole('button', { name: '核对下一项', exact: true }).click();
    await selectValue(main.getByRole('combobox', { name: '原文语言', exact: true }), 'zh-CN');
    await main.getByRole('group', { name: '待添加程序', exact: true }).getByRole('button').first().click();
    assert.equal(await main.getByRole('combobox', { name: '原文语言', exact: true }).getAttribute('data-select-value'), 'ja');
    assert.equal(await main.evaluate(() => window.uiHarness.commands.length), 0);
    await main.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await main.evaluate(() => window.uiHarness.state.games.length), 0);
    await open();
    await selectValue(main.getByRole('combobox', { name: '原文语言', exact: true }), 'ja');
    await main.getByRole('button', { name: '核对下一项', exact: true }).click();
    await selectValue(main.getByRole('combobox', { name: '原文语言', exact: true }), 'zh-CN');
    await main.evaluate(() => { window.uiHarness.failNextCommand = true; });
    await main.getByRole('button', { name: '添加到游戏库', exact: true }).click();
    await main.getByRole('alert').filter({ hasText: 'simulated save failure' }).waitFor();
    assert.equal(await main.evaluate(() => window.uiHarness.state.games.length), 0);
    assert.equal(await main.getByRole('textbox', { name: '游戏名称', exact: false }).inputValue(), 'UI batch B');
    const failed = await main.evaluate(() => window.uiHarness.commands.at(-1));
    assert.equal(failed.command.kind, 'save_games'); assert.equal(failed.command.payload.items.length, 2);
    assert.deepEqual(failed.command.payload.items.map(item => item.game.sourceLanguage), ['ja', 'zh-CN']);
    assert.equal(new Set(failed.command.payload.items.map(item => item.itemId)).size, 2);
    assert.ok(Number.isInteger(failed.expectedRevision));
    await main.getByRole('button', { name: '添加到游戏库', exact: true }).click();
    await main.getByRole('dialog').waitFor({ state: 'hidden' });
    const success = await main.evaluate(() => window.uiHarness.commands.at(-1));
    assert.deepEqual(success.command.payload.items, failed.command.payload.items);
    assert.equal(await main.evaluate(() => window.uiHarness.commands.length), 2);
    assert.equal(await main.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    assert.deepEqual(await main.evaluate(() => window.uiHarness.state.games.map(item => item.sourceLanguage)), ['ja', 'zh-CN']);
    await main.close();
  });
  await check('UI-49-session-auto-translation-is-independent-of-purpose-draft', async () => {
    const settings = await pageFor('settings','model-config'); await enterConfigurationSection(settings,'translation');
    const control = settings.getByRole('switch',{name:'当前会话自动翻译',exact:true});
    assert.equal(await control.isDisabled(),true);
    await settings.evaluate(() => { const port=window.uiHarness; port.publish({ session:{sessionId:'ui-auto-control',gameId:null,sourceKind:'hook',state:'active',paused:false,translationEnabled:true,sources:[]} }); });
    assert.equal(await control.isDisabled(),false); assert.equal(await control.isChecked(),true);
    const before=await settings.evaluate(()=>structuredClone(window.uiHarness.state.configuration));
    await settings.locator('label').filter({ has: control }).click();
    await settings.waitForFunction(()=>window.uiHarness.state.session.translationEnabled===false);
    const command=await settings.evaluate(()=>window.uiHarness.commands.at(-1).command);
    assert.deepEqual(command,{kind:'set_translation_enabled',payload:{enabled:false}});
    assert.deepEqual(await settings.evaluate(()=>window.uiHarness.state.configuration),before);
    assert.equal(await settings.evaluate(()=>window.uiHarness.commands.length),1);
    assert.equal(await settings.evaluate(()=>window.uiHarness.state.configuration.draft.bindings[0].enabled),false);
    assert.equal(await settings.evaluate(()=>window.uiHarness.state.configuration.applied),null);
    await control.focus(); await control.press('Space');
    await settings.waitForFunction(()=>window.uiHarness.state.session.translationEnabled===true);
    assert.deepEqual(await settings.evaluate(()=>window.uiHarness.commands.at(-1).command),{kind:'set_translation_enabled',payload:{enabled:true}});
    assert.deepEqual(await settings.evaluate(()=>window.uiHarness.state.configuration),before);
    assert.equal(await settings.evaluate(()=>window.uiHarness.commands.length),2);
    await settings.close();
  });
  await check('UI-50-in-sheet-drop-preserves-dirty-and-batch-drafts',async()=>{
    const main=await pageFor();
    await main.getByRole('button',{name:'添加游戏…',exact:true}).first().click();
    await main.evaluate(()=>window.uiHarness.emitDrop(['/UI-test/First.exe']));
    await main.getByRole('textbox',{name:'游戏名称',exact:false}).waitFor();
    await main.waitForFunction(()=>document.querySelector('input[required]')?.value==='First.exe');
    assert.equal(await main.evaluate(()=>window.uiHarness.commands.length),0);
    await main.evaluate(()=>window.uiHarness.emitDrop(['/UI-test/Replacement.exe']));
    await main.getByRole('dialog').getByRole('alert').filter({hasText:'当前资料尚未保存'}).waitFor();
    assert.equal(await main.getByRole('textbox',{name:'游戏名称',exact:false}).inputValue(),'First.exe');
    assert.equal(await main.evaluate(()=>window.uiHarness.commands.length),0);
    await main.getByRole('button',{name:'取消',exact:true}).click();
    await main.evaluate(()=>window.uiHarness.emitDrop(['/UI-test/Retry.exe']));
    await main.waitForFunction(()=>document.querySelector('input[required]')?.value==='Retry.exe');
    assert.equal(await main.getByRole('alert').count(),0);
    await main.keyboard.press('Escape');await main.getByRole('dialog').waitFor({state:'hidden'});
    await main.getByRole('button',{name:'添加游戏…',exact:true}).first().click();
    await main.getByRole('textbox',{name:'游戏名称',exact:false}).fill('手写草稿');
    await main.evaluate(()=>window.uiHarness.emitDrop(['/UI-test/Overwritten.exe']));
    await main.getByRole('dialog').getByRole('alert').filter({hasText:'当前资料尚未保存'}).waitFor();
    assert.equal(await main.getByRole('textbox',{name:'游戏名称',exact:false}).inputValue(),'手写草稿');
    await main.getByRole('button',{name:'取消',exact:true}).click();
    await main.evaluate(()=>{window.uiHarness.executableSelections=['First','Second'].map(name=>({path:`/UI-test/${name}.exe`,displayName:name,architecture:'x86',environmentKey:'crossover:/UI-test/Bottle',bottleLocator:'/UI-test/Bottle'}));});
    await main.getByRole('button',{name:'添加游戏…',exact:true}).first().click();await main.getByRole('button',{name:'选择文件…',exact:true}).click();
    const names=await main.getByRole('group',{name:'待添加程序',exact:true}).getByRole('button').allTextContents();
    await main.evaluate(()=>window.uiHarness.emitDrop(['/UI-test/Overwritten.exe']));
    await main.getByRole('dialog').getByRole('alert').filter({hasText:'当前资料尚未保存'}).waitFor();
    assert.deepEqual(await main.getByRole('group',{name:'待添加程序',exact:true}).getByRole('button').allTextContents(),names);
    assert.equal(await main.getByRole('textbox',{name:'游戏名称',exact:false}).inputValue(),'First');
    await main.getByRole('button',{name:'取消',exact:true}).click();
    assert.equal(await main.evaluate(()=>window.uiHarness.commands.length),0);assert.equal(await main.evaluate(()=>window.uiHarness.state.games.length),0);assert.equal(await main.evaluate(()=>(window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)),0);
    await main.close();
  });
await check('UI-51-configuration-tabs-retain-draft-and-explicit-test-target',async()=>{
  const p=await pageFor('settings','model-config');
  assert.equal(await p.getByRole('spinbutton',{name:'请求超时秒数',exact:true}).count(),0);
  await enterConfigurationRuntime(p,'purpose');
  assert.equal(await p.getByRole('combobox',{name:'测试对象',exact:true}).getAttribute('data-select-value'),'purpose');
  await p.getByRole('spinbutton',{name:'请求超时秒数',exact:true}).fill('63');
  await enterConfigurationSection(p,'service-advanced');
  await enterConfigurationRuntime(p,'connection');
  assert.equal(await p.getByRole('combobox',{name:'测试对象',exact:true}).getAttribute('data-select-value'),'connection');
  assert.equal(await p.getByRole('spinbutton',{name:'请求超时秒数',exact:true}).inputValue(),'63');
  await chooseConfigurationService(p,'ui-model-b');await enterConfigurationRuntime(p,'model');
  assert.equal(await p.getByRole('combobox',{name:'测试对象',exact:true}).getAttribute('data-select-value'),'model');
  assert.equal(await p.getByRole('spinbutton',{name:'请求超时秒数',exact:true}).inputValue(),'63');
  assert.equal(await p.evaluate(()=>window.uiHarness.commands.length),0);await p.close();
});
await check('UI-52-connection-copy-cancel-and-explicit-credential-reference',async()=>{const p=await pageFor('settings','model-config');await p.evaluate(()=>{const port=window.uiHarness,draft=structuredClone(port.state.configuration.draft);draft.connections[0].credentialPoolId='focused-existing-reference';draft.connections[0].credentialPoolRevision=7;port.publish({configuration:{...port.state.configuration,draft}});});await enterConfigurationSection(p,'service-advanced');const source=await p.evaluate(()=>structuredClone(window.uiHarness.state.configuration.draft.connections[0]));await p.getByRole('button',{name:'复制服务连接…',exact:true}).click();assert.equal(await p.getByRole('switch',{name:'复用此连接的系统凭据引用',exact:true}).isChecked(),false);await p.getByRole('button',{name:'取消',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.commands.length),0);await p.getByRole('button',{name:'复制服务连接…',exact:true}).click();await p.getByRole('textbox',{name:'连接副本名称',exact:true}).fill('独立副本');await p.getByRole('button',{name:'创建连接副本',exact:true}).click();assert.equal(await p.getByRole('textbox',{name:'连接名称',exact:true}).inputValue(),'独立副本');await p.getByRole('button',{name:'保存草稿',exact:true}).click();await p.getByText('草稿已保存',{exact:true}).waitFor();const saved=await p.evaluate(()=>window.uiHarness.commands.at(-1));assert.equal(saved.command.kind,'save_configuration');const c=saved.command.payload.configuration.connections;assert.equal(c.length,2);assert.deepEqual(c[0],source);assert.notEqual(c[1].connectionId,c[0].connectionId);assert.equal(c[1].credentialPoolId,null);assert.equal(c[1].credentialPoolRevision,null);await p.getByRole('button',{name:'复制服务连接…',exact:true}).click();await p.getByRole('button',{name:'取消',exact:true}).click();await chooseConfigurationConnection(p, source.connectionId);await p.getByRole('button',{name:'复制服务连接…',exact:true}).click();await p.getByRole('textbox',{name:'连接副本名称',exact:true}).fill('显式复用副本');await p.getByRole('switch',{name:'复用此连接的系统凭据引用',exact:true}).check();await p.getByRole('button',{name:'创建连接副本',exact:true}).click();await p.getByRole('button',{name:'保存草稿',exact:true}).click();await p.getByText('草稿已保存',{exact:true}).waitFor();const explicit=await p.evaluate(()=>window.uiHarness.commands.at(-1).command.payload.configuration.connections.at(-1));assert.equal(explicit.credentialPoolId,source.credentialPoolId);assert.equal(explicit.credentialPoolRevision,7);await p.close();});
await check('UI-53-draft-removal-preserves-applied-history-and-credentials',async()=>{const p=await pageFor('settings','model-config');await p.evaluate(()=>{const port=window.uiHarness;const draft=structuredClone(port.state.configuration.draft);draft.templates=[{templateId:'owned-delete',purpose:'translation',displayName:'owned',sourceKind:'custom',sourceVersion:null,body:{user:'{{source}}'},variables:['source'],compatibility:{modelConfigurationId:draft.models[0].modelId}}];draft.models[0].defaultTemplates={translation:'owned-delete'};port.publish({configuration:{...port.state.configuration,draft,applied:structuredClone(draft)}});});const original=await p.evaluate(()=>structuredClone(window.uiHarness.state.configuration));await enterConfigurationSection(p,'service-advanced');await p.getByRole('button',{name:'从草稿移除连接…',exact:true}).click();await p.getByText('已应用版本仍引用此对象；请在“测试与运行”中先停用，再检查并应用新草稿。此处不会原子删除或停用已应用版本。',{exact:true}).waitFor();await p.getByRole('button',{name:'取消',exact:true}).click();assert.equal(await p.evaluate(()=>window.uiHarness.commands.length),0);await p.getByRole('button',{name:'从草稿移除连接…',exact:true}).click();await p.getByRole('button',{name:'从草稿移除',exact:true}).click();await p.getByRole('button',{name:'保存草稿',exact:true}).click();await p.getByText('草稿已保存',{exact:true}).waitFor();const state=await p.evaluate(()=>({command:window.uiHarness.commands.at(-1),applied:window.uiHarness.state.configuration.applied}));assert.equal(state.command.command.kind,'save_configuration');const next=state.command.command.payload.configuration;assert.equal(next.connections.length,0);assert.equal(next.models.length,0);assert.equal(next.templates.length,0);assert.ok(next.bindings.every(b=>b.engines.length===0&&!b.enabled));assert.deepEqual(state.applied,original.applied);await p.close();});
await check('UI-54-model-enumeration-is-connection-test-not-save-or-apply',async()=>{const p=await pageFor('settings','model-config');await enterConfigurationSection(p,'service-advanced');const before=await p.evaluate(()=>({configuration:structuredClone(window.uiHarness.state.configuration),calls:window.uiHarness.configurationTests.length}));await p.evaluate(()=>window.uiHarness.configurationTestResponses.push({error:'focused model enumeration failure'}));await p.getByRole('button',{name:'刷新列表',exact:true}).click();await p.getByRole('alert').filter({hasText:'focused model enumeration failure'}).waitFor();assert.equal(await p.getByRole('textbox',{name:'实际模型 ID',exact:true}).inputValue(),before.configuration.draft.models[0].actualModelId);const after=await p.evaluate(()=>({configuration:window.uiHarness.state.configuration,calls:window.uiHarness.configurationTests,commands:window.uiHarness.commands}));assert.equal(after.calls.length,before.calls+1);assert.equal(after.calls.at(-1).objectKind,'connection');assert.equal(after.calls.at(-1).objectId,before.configuration.draft.models[0].connectionId);assert.equal(after.commands.length,0);assert.deepEqual(after.configuration.applied,before.configuration.applied);await p.getByRole('textbox',{name:'实际模型 ID',exact:true}).fill('manual-after-error');assert.equal(await p.getByRole('button',{name:'刷新列表',exact:true}).isDisabled(),true);await p.close();});

  await check('UI-55-hud-actions-retain-source-and-patch-only-selected-preference', async () => {
    const p = await pageFor('hud', 'reading', { width: 560, height: 500 });
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.current));
    const open = async () => { await p.getByRole('button', { name: '更多阅读操作', exact: true }).click(); await p.getByRole('menu').waitFor(); };
    await open(); assert.equal(await p.getByRole('menu').getByRole('menuitem').count(), 5);
    assert.deepEqual(await p.getByRole('menuitem').allTextContents(), ['设置…', '选择或更换游戏窗口…', '解除窗口关联', '恢复浮窗位置', '断开游戏连接…']);
    await p.keyboard.press('Escape');
    await p.getByRole('button', { name: '可点击分词', exact: true }).click();
    await p.waitForFunction(() => window.uiHarness.state.preferences.segmentationEnabled === false);
    assert.equal(await p.getByTestId('source-text').getByRole('button').count(), 0);
    assert.ok(await p.getByTestId('source-text').locator('rt').count() > 0, 'Ruby remains independently enabled without clickable segmentation.');
    assert.equal(await p.getByTestId('source-text').evaluate(node => { const clone=node.cloneNode(true);clone.querySelectorAll('rt').forEach(rt=>rt.remove());return clone.textContent; }), before.sourceText);
    assert.deepEqual(await p.evaluate(() => window.uiHarness.state.current), before);
    await p.getByRole('button', { name: '可点击分词', exact: true }).click();
    await p.getByTestId('source-text').getByRole('button').first().waitFor();
    await p.getByRole('button', { name: '切换紧凑布局', exact: true }).click();
    await p.waitForFunction(() => getComputedStyle(document.querySelector('[data-testid=source-text]')).fontSize === '16px');
    assert.equal(await p.evaluate(() => window.uiHarness.state.preferences.sourceFontPt), 18);
    await p.getByRole('button', { name: '取消置顶', exact: true }).click();
    await p.waitForFunction(() => window.uiHarness.state.preferences.overrides['hud.alwaysOnTop'] === false);
    await p.getByRole('button', { name: '重新翻译本句', exact: true }).click();
    const after = await p.evaluate(() => ({ commands: window.uiHarness.commands, retranslations: window.uiHarness.retranslations, current: window.uiHarness.state.current }));
    assert.deepEqual(after.commands.map(x => x.command.kind), Array(4).fill('set_hud_preference'));
    assert.deepEqual(after.commands.map(x => x.command.payload.preference), ['segmentation', 'segmentation', 'compact', 'always_on_top']);
    assert.equal(after.retranslations.at(-1).unitId, before.unitId); assert.equal(after.retranslations.at(-1).textRevision, before.textRevision);
    assert.deepEqual(after.current, before); await p.close();
  });
  await check('UI-56-main-navigation-dismisses-unconfirmed-capture-and-retains-library-draft', async () => {
    const p = await pageFor('main', 'reading');
    await p.getByRole('button', { name: '添加游戏…', exact: true }).first().click();
    await p.getByRole('textbox', { name: '游戏名称', exact: false }).fill('未保存的游戏资料');
    const before = await p.evaluate(() => structuredClone(window.uiHarness.state.session));
    await p.evaluate(() => window.uiHarness.openMainPage('history'));
    await p.getByRole('heading', { name: '历史记录', exact: true }).waitFor();
    assert.equal(await p.getByRole('dialog').count(), 0);
    await p.getByRole('button', { name: '游戏库', exact: true }).click();
    assert.equal(await p.getByRole('textbox', { name: '游戏名称', exact: false }).inputValue(), '未保存的游戏资料');
    await p.getByRole('button', { name: '取消', exact: true }).click();
    await p.evaluate(() => window.uiHarness.openMainPage('capture'));
    await p.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).waitFor();
    await p.evaluate(() => window.uiHarness.openMainPage('history'));
    await p.getByRole('heading', { name: '历史记录', exact: true }).waitFor();
    assert.equal(await p.getByRole('dialog').count(), 0);
    assert.deepEqual(await p.evaluate(() => window.uiHarness.state.session), before);
    assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    assert.equal(await p.evaluate(() => window.uiHarness.commands.length), 0); await p.close();
  });
  // UI-57 old launch journey is superseded by the user's running-game-only
  // decision in repair-library-binding-preview-diagnostics; original bytes are archived.
  await check('UI-57-running-game-only-entrypoints-and-one-confirmation', async () => {
    const p = await pageFor();
    await p.evaluate(({ game, candidate }) => { const port = window.uiHarness; port.publish({ games: [game] }); port.targetCandidates = [candidate];port.bindingJourneyFixture.autoMatchGameId=game.gameId; }, { game: designLibraryGame, candidate: designTarget });
    for (const name of ['运行并连接…', '关联并接入…', '连接已运行的游戏…']) assert.equal(await p.getByRole('button', { name, exact: true }).count(), 0);
    await p.getByRole('button', { name: '连接运行中的游戏…', exact: true }).first().click();
    await p.getByRole('radio').first().check();
    await p.getByRole('region', { name: '关联与实际取文目标', exact: true }).waitFor();
    assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 0);
    assert.equal(await p.getByRole('button', { name: '核对目标', exact: true }).count(), 0);
    await p.getByRole('button', { name: '关联并接入', exact: true }).click();
    await p.getByRole('heading', { name: '正在接入游戏', exact: true }).waitFor();
    assert.equal(await p.evaluate(() => (window.uiHarness.associations.length + window.uiHarness.reviewedAssociations.length)), 1);
    assert.deepEqual(await p.evaluate(() => window.uiHarness.launches), []);
    assert.equal(await p.evaluate(() => window.uiHarness.commands.filter(item => item.command.kind === 'select_source').length), 0);
    const entrySource = await fs.readFile(path.join(root, 'apps/desktop/src-tauri/src/main.rs'), 'utf8');
    assert.equal(entrySource.includes('application_launch_game'), false, 'Public client must expose no game launch IPC');
    await p.close();
  });
  await captureDesignStates();
  }

} catch (error) { behavior.push({ id: 'UI-runner', status: 'failed', message: error.stack }); }
finally { await browser?.close(); child.kill('SIGTERM'); await fs.writeFile(path.join(out, 'vite.log'), serverLog); }
const sourceDrift = [];
for (const [file, before] of Object.entries(sourceSha256)) { try { if (hash(await fs.readFile(path.join(root, file))) !== before) sourceDrift.push(file); } catch { sourceDrift.push(file); } }
if (sourceDrift.length) behavior.push({ id: 'UI-source-identity', status: 'failed', message: `Source changed during execution: ${sourceDrift.join(', ')}` });
if (productionSecurityPolicy.violations.length) behavior.push({ id: 'UI-production-security-policy', status: 'failed', message: 'Actual document rendering violated the production Content Security Policy.', violations: productionSecurityPolicy.violations });
const behaviorStatus = behavior.length > 0 && behavior.every(item => item.status === 'passed') ? 'passed' : 'failed';
const visualStatus = captures.length > 0 && captures.every(item => item.status === 'passed') ? 'passed' : 'failed';
const report = { version: 1, executionNonce: process.env.LUNA_PRODUCT_EXECUTION_NONCE ?? null, browser: { version: browserVersion, playwright: require('playwright-core/package.json').version, executableOverride: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ?? null, defaultEngine: 'Playwright-managed pinned headless Chromium' }, productionSecurityPolicy, sourceSha256, designSha256, sourceDrift, nativeRoleContract: { source: nativeWindowSource, sourceSha256: hash(nativeSource), urlTemplate: nativeUrlTemplate }, executedAt: new Date().toISOString(), status: behaviorStatus === 'passed' && visualStatus === 'passed' && !proposalPreview ? 'passed' : 'failed', targetedCaptureIds: captureOnly?[...captureOnly]:null, referenceCatalogueSha256: referenceCatalogue?.sha256 ?? null, designSourceVersion: referenceCatalogue?.designSourceVersion ?? null, referenceCatalogue: referenceCatalogue ? {file:referenceCatalogue.file,sha256:referenceCatalogue.sha256,status:referenceCatalogue.status,proposalPreview} : null, behaviorStatus, visualStatus, evidenceKind: 'production React components with explicitly controlled test-only ApplicationPort', limitations: ['Not a packaged Tauri/native compositor run.', 'No real Hook injection, game, model, Keychain, tokenizer, or dictionary execution.', 'Design references are read-only existing PNG copies; full-layout review remains pending.', 'Dimension differences and pixel differences remain failures; no approved screenshot replacement, masks, resizing, or threshold increase.'], behavior, captures, browserErrors: errors };
await fs.writeFile(path.join(out, 'verification.json'), JSON.stringify(report, null, 2) + '\n');
const escape = text => String(text).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
await fs.writeFile(path.join(out, 'index.html'), `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>Luna 产品UI候选检查</title><style>body{font:14px/1.8 -apple-system,sans-serif;background:#20232c;color:#e9e9ef;max-width:1400px;margin:40px auto;padding:0 25px}h1{font-size:26px}section{margin:30px 0;padding:20px;background:#353641;border-radius:13px}.images{display:grid;grid-template-columns:repeat(3,1fr);gap:15px}img{max-width:100%;border:1px solid #8883}small,p{color:#b0b2c0}table{border-collapse:collapse;width:100%}td,th{padding:8px;text-align:left;border-bottom:1px solid #8883}@media(max-width:800px){.images{grid-template-columns:1fr}}</style><h1>首版产品UI候选与设计差异</h1><p>行为检查 ${behaviorStatus} · 视觉比较 ${visualStatus}。仅验证真实 React 组件与受控测试端口；不代表正式包、游戏或模型功能通过。</p><table><thead><tr><th>检查</th><th>结果</th><th>说明</th></tr></thead><tbody>${behavior.map(item => `<tr><td>${escape(item.id)}</td><td>${escape(item.status)}</td><td>${escape(item.message ?? '')}</td></tr>`).join('')}</tbody></table>${captures.map(item => item.status === 'blocked_pending_replacement_reference' ? `<section><h2>${escape(item.id)} · ${escape(item.status)}</h2><p>${escape(item.reason)}</p><p>保留 ${1 + (item.variants?.length ?? 0)} 个原始参照变体；当前前提已退役，尚未捕获候选或计算像素差异。</p></section>` : `<section><h2>${escape(item.id)} · ${item.status}</h2><p>${escape(item.reference)}<br/>基线 ${item.expectedSize.join('×')}，候选 ${item.actualSize.join('×')}；差异 ${(item.ratio * 100).toFixed(2)}%。${escape(item.reason)}</p><div class="images"><figure><img src="${item.baselineCopy}"><figcaption>原有设计截图只读副本</figcaption></figure><figure><img src="${item.candidate}"><figcaption>实际产品组件候选</figcaption></figure><figure><img src="${item.diff}"><figcaption>像素差异（不同尺寸只在输出画布补透明）</figcaption></figure></div></section>`).join('')}<p>本次没有覆盖设计源或已确认图，也没有将未执行的正式产品验收改为通过。</p></html>`);
process.stdout.write(JSON.stringify({ status: report.status, behaviorStatus, visualStatus, report: path.relative(root, path.join(out, 'verification.json')), behaviorChecks: behavior.length, visualStates: captures.length }) + '\n');
process.exitCode = report.status === 'passed' ? 0 : 1;
