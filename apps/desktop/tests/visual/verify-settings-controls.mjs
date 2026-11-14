/** Real production React, controlled ApplicationPort. No native/login-item changes. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { createHash, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import nodeAssert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const desktop = path.join(root, 'apps/desktop');
const argument = name => process.argv[process.argv.indexOf(name) + 1];
const out = path.resolve(root, process.argv.includes('--output') ? argument('--output') : `.build/settings-controls/qa-${Date.now()}`);
const nonce = process.argv.includes('--nonce') ? argument('--nonce') : randomUUID();
if (!out.startsWith(path.join(root, '.build') + path.sep) || fsSync.existsSync(out)) throw Error('Fresh repository .build output required');
await fs.mkdir(out, { recursive: true });
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
let assertionCount = 0;
const assert = new Proxy(nodeAssert, { apply(fn, self, args) { assertionCount++; return Reflect.apply(fn, self, args); }, get(fn, key) { const value = fn[key]; return typeof value === 'function' ? (...args) => { assertionCount++; return Reflect.apply(value, fn, args); } : value; } });
const require = createRequire(path.join(root, 'package.json'));
const { chromium } = require('playwright-core');
async function execute(command, cwd = root) {
  const child = spawn(command[0], command.slice(1), { cwd, stdio: ['ignore', 'pipe', 'pipe'] }); let stdout = '', stderr = '';
  child.stdout.on('data', data => { stdout += data; }); child.stderr.on('data', data => { stderr += data; });
  const exitCode = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); }); return { command, exitCode, stdout, stderr };
}
async function sourceHashes() {
  const result = await execute(['python3', '-c', "import json,sys;from pathlib import Path;sys.path.insert(0,'scripts/quality');from gate_core import source_files;print(json.dumps(source_files(Path.cwd()),sort_keys=True))"]);
  if (result.exitCode !== 0) throw Error(result.stderr); return JSON.parse(result.stdout);
}
const before = await sourceHashes(), executedAt = new Date().toISOString();
const csp = JSON.parse(await fs.readFile(path.join(desktop, 'src-tauri/tauri.conf.json'), 'utf8')).app.security.csp;
const checks = [], captures = [], builds = [], errors = [], externalRequests = [];
let browser, server, origin, active;
async function runBuild(args, name) {
  const log = await execute([process.execPath, ...args], desktop); await fs.writeFile(path.join(out, name), JSON.stringify(log, null, 2) + '\n');
  builds.push({ command: log.command, exitCode: log.exitCode, log: name }); assert.equal(log.exitCode, 0, log.stderr);
}
const login = (status, extra = {}) => ({ status, canChange: true, errorCode: null, detail: status === 'requires_approval' ? '已登记，尚未生效，等待系统批准；请前往系统设置 → 通用 → 登录项，允许 Luna 在登录后打开。' : null, ...extra });
async function app(scenario = 'model-config', initial = {}) {
  const p = await browser.newPage({ viewport: { width: 1160, height: 900 }, deviceScaleFactor: 1 }); p.setDefaultTimeout(6000);
  p.on('pageerror', error => errors.push(error.message)); p.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await p.route('**/*', route => { const url = new URL(route.request().url()); if (url.origin !== origin && !['data:', 'blob:'].includes(url.protocol)) { externalRequests.push(url.href); return route.abort(); } return route.continue(); });
  await p.addInitScript({ content: `window.__initializeControlledPort = port => { const f=${JSON.stringify(initial)}; if(f.login)port.loginStartupFixture=f.login;if(f.patch)port.state={...port.state,...f.patch}; };` });
  const response = await p.goto(origin + `/tests/visual/index.html?role=settings&scenario=${scenario}`); assert.equal(await response.headerValue('content-security-policy'), csp);
  await p.waitForFunction(() => window.uiHarness && document.getElementById('root')?.children.length);
  await p.evaluate(() => Object.assign(document.getElementById('root').style, { width: '100%', height: '100%', left: '0', top: '0' }));
  active = p; return p;
}
async function general(scenario = 'reading', initial = {}) {
  const p = await app(scenario, { login: login('not_registered'), ...initial }); await p.getByRole('button', { name: '通用', exact: true }).click(); await p.locator('[data-ui-page="general-settings"]').waitFor();
  await p.waitForFunction(() => window.uiHarness.loginStartupQueries >= 1); return p;
}
const button = (p, name) => p.getByRole('button', { name, exact: true });
const field = (p, name) => p.getByRole('textbox', { name, exact: true });
async function disclosure(p, id, open = true) { const d = p.locator(`details[data-configuration-section="${id}"]`); if ((await d.getAttribute('open') !== null) !== open) await d.locator(':scope > summary').click(); return d; }
async function select(p, name, value) { const t = p.getByRole('combobox', { name, exact: true }); await t.click(); const o = p.getByRole('option'); const index = await o.evaluateAll((nodes, target) => nodes.findIndex(node => node.getAttribute('data-select-value') === target), value); assert.ok(index >= 0, `${name} visible option: ${value}`); await o.nth(index).click(); }
async function edit(p, name = 'UI model A', orphan = false) { await p.getByRole('button', { name: `${orphan ? '编辑连接草稿' : '编辑服务'}：${name}`, exact: true }).click(); }
async function state(p) { return p.evaluate(() => ({ configuration: structuredClone(window.uiHarness.state.configuration), session: structuredClone(window.uiHarness.state.session), commands: structuredClone(window.uiHarness.commands), tests: structuredClone(window.uiHarness.configurationTests), loginQueries: window.uiHarness.loginStartupQueries, loginRequests: structuredClone(window.uiHarness.loginStartupRequests), loginFixture: structuredClone(window.uiHarness.loginStartupFixture), openedMainPages: structuredClone(window.uiHarness.mainPageRequests.map(request => request.intent)) })); }
async function toggle(control, selected) { if (await control.isChecked() !== selected) { await control.focus(); await control.press('Space'); } }
async function tick(p) { await p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function save(p) { await button(p, '保存草稿').click(); await p.getByText('草稿已保存', { exact: true }).waitFor(); }
async function switchLabelGeometry(p) {
  const labels = p.locator('[data-ui-page="general-settings"] [data-switch-label]').filter({ has: p.locator('small') });
  const rows = [];
  for (let index = 0; index < await labels.count(); index++) {
    const label = labels.nth(index); await label.scrollIntoViewIfNeeded();
    const row = await label.evaluate(node => {
      const strong = node.querySelector('strong'), small = node.querySelector('small'), container = node.parentElement;
      const control = container.querySelector('[role="switch"]'), track = container.querySelector(':scope > span:last-child');
      const rectangle = element => { const r = element.getBoundingClientRect(); return { left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height }; };
      const describedBy = (control.getAttribute('aria-describedby') ?? '').trim().split(/\s+/).filter(Boolean);
      return { name: control.getAttribute('aria-label'), title: strong.textContent, description: small.textContent,
        descriptionId: small.id, describedBy, descriptionLinked: describedBy.includes(small.id) && document.getElementById(small.id) === small,
        strong: rectangle(strong), small: rectangle(small), track: rectangle(track) };
    });
    for (const key of ['strong', 'small', 'track']) { assert.ok(Object.values(row[key]).every(Number.isFinite)); assert.ok(row[key].width > 0 && row[key].height > 0, `${row.name}: ${key} has positive area`); }
    assert.ok(row.small.top >= row.strong.bottom, `${row.name}: description must follow the title on a separate line`);
    for (const key of ['strong', 'small']) assert.equal(Math.min(row[key].right, row.track.right) > Math.max(row[key].left, row.track.left) && Math.min(row[key].bottom, row.track.bottom) > Math.max(row[key].top, row.track.top), false, `${row.name}: switch track must not cover ${key}`);
    assert.equal(row.descriptionLinked, true); assert.equal(row.title, row.name); rows.push(row);
  }
  assert.deepEqual(rows.map(row => row.name).sort(), ['新台词自动加入当前会话', '可点击日语分词', '显示日语注音', '显示词性标记'].sort());
  return rows;
}
async function check(id, run) {
  const start = assertionCount; active = null; const record = { id, status: 'failed', assertionCount: 0 }; const raw = { schemaVersion: 1, id, nonce, inputs: null, actions: [], outputs: null };
  const ctx = { input: value => { raw.inputs = value; }, action: value => { raw.actions.push(value); }, output: value => { raw.outputs = value; } };
  try { await run(ctx); record.status = 'passed'; } catch (error) { record.error = error.message; record.stack = error.stack; }
  finally {
    record.assertionCount = assertionCount - start;
    if (active && !active.isClosed()) {
      raw.finalDom = await active.locator('body').evaluate(node => node.outerHTML); const png = `${id}.candidate.png`; await active.screenshot({ path: path.join(out, png), animations: 'disabled' });
      const dom = `${id}.dom.html`; await fs.writeFile(path.join(out, dom), raw.finalDom); delete raw.finalDom;
      const capture = { file: png, sha256: sha(await fs.readFile(path.join(out, png))), domFile: dom, domSha256: sha(await fs.readFile(path.join(out, dom))), viewport: active.viewportSize(), producer: 'production_react_controlled_transport', approvedReference: false, nativeVerified: false }; captures.push(capture); record.capture = capture;
      await active.close();
    }
    const file = `${id}.raw.json`; await fs.writeFile(path.join(out, file), JSON.stringify(raw, null, 2) + '\n'); record.raw = { file, sha256: sha(await fs.readFile(path.join(out, file))) }; checks.push(record); console.log(`${id}: ${record.status}${record.error ? ': ' + record.error : ''}`);
  }
}
try {
  await runBuild([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck-tests.json');
  await runBuild([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--outDir', path.join(out, 'production-build'), '--sourcemap'], 'production-build.json');
  const build = path.join(out, 'controlled-build');
  await runBuild([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(desktop, 'tests/visual/vite.config.ts'), '--outDir', build, '--sourcemap'], 'component-build.json');
  server = http.createServer(async (req, res) => { try { const file = path.resolve(build, '.' + new URL(req.url, 'http://localhost').pathname); if (!file.startsWith(build + path.sep)) throw Error('outside build'); const bytes = await fs.readFile(file), ext = path.extname(file); res.writeHead(200, { 'content-type': ({ '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml' })[ext] ?? 'application/octet-stream', ...(ext === '.html' ? { 'content-security-policy': csp } : {}) }); res.end(bytes); } catch { res.writeHead(404); res.end(); } });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disable-gpu', '--force-color-profile=srgb'] });
  await check('SC-01-workspace-three-protocols-and-exact-save', async c => {
    const p = await app(); const input = await state(p); c.input(input);
    assert.equal(await p.locator('[data-service-workspace]').count(), 1); assert.equal(await p.getByRole('group', { name: '模型配置实体', exact: true }).count(), 0);
    for (const name of ['用途', '连接', '模型', '模板与参数', '测试与运行', '术语表']) assert.equal(await button(p, name).count(), 0, 'Retired entity navigation must not be hidden in production');
    await button(p, '添加服务…').click(); c.action('open-add-service'); const sheet = p.getByRole('dialog'); await sheet.getByRole('combobox', { name: '服务接口', exact: true }).click(); const protocols = await p.getByRole('option').evaluateAll(nodes => nodes.map(node => ({ value: node.getAttribute('data-select-value'), label: node.textContent }))); assert.deepEqual(protocols.map(item => item.value), ['openai_compatible', 'anthropic', 'azure_translator']); await p.keyboard.press('Escape');
    const addServiceReview = { invalidNames: ['', '   '], disabled: [], blockedClicks: [], rowCountBefore: await p.locator('[data-service-model],[data-service-connection]').count() };
    for (const value of addServiceReview.invalidNames) {
      await sheet.getByRole('textbox', { name: '新服务名称', exact: true }).fill(value);
      const create = button(p, '创建服务草稿'), disabled = await create.isDisabled(); addServiceReview.disabled.push(disabled); assert.equal(disabled, true);
      let rejection = ''; try { await create.click({ timeout: 250 }); } catch (error) { rejection = error.message; }
      addServiceReview.blockedClicks.push(rejection); assert.ok(rejection.toLowerCase().includes('not enabled')); c.action('empty-name-create-rejected:' + JSON.stringify(value));
    }
    await button(p, '取消').click(); c.action('cancel-add-service');
    await p.waitForFunction(() => document.activeElement?.textContent?.trim() === '添加服务…');
    addServiceReview.focusReturned = await button(p, '添加服务…').evaluate(node => node === document.activeElement);
    addServiceReview.rowCountAfter = await p.locator('[data-service-model],[data-service-connection]').count();
    assert.equal(addServiceReview.focusReturned, true); assert.equal(addServiceReview.rowCountAfter, addServiceReview.rowCountBefore);
    assert.deepEqual((await state(p)).configuration.draft, input.configuration.draft);
    await edit(p); await field(p, '模型配置名称').fill('SC precise model'); c.action('edit-model-name:SC precise model'); await save(p); c.action('save-configuration');
    const output = await state(p), expected = structuredClone(input.configuration.draft); expected.models[0].displayName = 'SC precise model'; expected.revisionId = output.commands.at(-1).command.payload.configuration.revisionId;
    assert.equal(output.commands.at(-1).command.kind, 'save_configuration'); assert.deepEqual(output.commands.at(-1).command.payload.configuration, expected); assert.equal(output.commands.at(-1).command.payload.expectedHeadRevision, input.configuration.draftHeadRevision); assert.deepEqual(output.configuration.applied, input.configuration.applied);
    c.output({ ...output, protocols, addServiceReview, retiredNavigationCount: await p.getByRole('group', { name: '模型配置实体', exact: true }).count() });
  });
  await check('SC-02-shared-connection-and-orphan-draft-reachable', async c => {
    const p = await app(); await p.evaluate(() => { const port = window.uiHarness, draft = structuredClone(port.state.configuration.draft); draft.connections.push({ ...draft.connections[0], connectionId: 'sc-orphan', displayName: 'SC orphan' }); port.publish({ configuration: { ...port.state.configuration, draft } }); }); c.input(await state(p));
    const seen = []; for (const name of ['UI model A', 'UI model B']) { await edit(p, name); seen.push(await field(p, '实际模型 ID').inputValue()); c.action('edit:' + name); }
    await edit(p, 'SC orphan', true); c.action('edit-orphan'); await disclosure(p, 'service-advanced'); const orphan = await field(p, '连接名称').inputValue(); assert.deepEqual(seen, ['ui-entered-A', 'ui-entered-B']); assert.equal(orphan, 'SC orphan'); assert.equal((await state(p)).commands.length, 0); c.output({ seen, orphan, ...(await state(p)) });
  });
  await check('SC-03-collapse-and-service-switch-preserve-pool-draft', async c => {
    const p = await app(); await p.evaluate(() => { const port = window.uiHarness; port.publish({ credentials: [{ credentialId: 'sc-credential', maskedLabel: 'SC masked', state: 'active', lifecycleRevision: 1 }] }); }); c.input(await state(p));
    await edit(p); await select(p, '认证方式', 'api_key'); await disclosure(p, 'service-advanced'); await select(p, '密钥选择策略', 'round_robin'); await p.getByRole('switch', { name: '加入此池', exact: true }).check(); c.action('edit-unsaved-pool:round_robin:sc-credential');
    await disclosure(p, 'service-advanced', false); await edit(p, 'UI model B'); await edit(p); await disclosure(p, 'service-advanced'); c.action('collapse-switch-B-return-A');
    const policy = await p.getByRole('combobox', { name: '密钥选择策略', exact: true }).getAttribute('data-select-value'), member = await p.getByRole('switch', { name: '加入此池', exact: true }).isChecked(); assert.equal(policy, 'round_robin'); assert.equal(member, true); assert.equal((await state(p)).commands.length, 0); c.output({ policy, member, ...(await state(p)) });
  });
  await check('SC-04-explicit-test-target-and-failure-preserve-lifecycle', async c => {
    const p = await app(); c.input(await state(p)); await edit(p); await disclosure(p, 'testing'); await select(p, '测试对象', 'model'); await p.evaluate(() => window.uiHarness.configurationTestResponses.push({ error: 'SC explicit test failure' })); c.action('select-model-test-and-inject-failure');
    await button(p, '测试当前对象').click(); await p.getByRole('alert').filter({ hasText: 'SC explicit test failure' }).waitFor(); c.action('test-current-object'); const output = await state(p); assert.equal(output.tests.at(-1).objectKind, 'model'); assert.equal(output.tests.at(-1).objectId, 'ui-model-a'); assert.equal(output.commands.length, 0); c.output({ ...output, errorVisible: true });
  });
  await check('SC-05-advanced-purpose-learning-terms-reachable', async c => {
    const p = await app('model-config', { patch: { session: { sessionId: 'sc-purpose-session', gameId: null, sourceKind: 'hook', state: 'active', paused: false, translationEnabled: true, sources: [] } } }); c.input(await state(p)); await edit(p); const visible = {};
    for (const id of ['service-advanced', 'translation', 'learning', 'testing', 'terms']) { await disclosure(p, id); c.action('expand:' + id); visible[id] = await p.locator(`details[data-configuration-section="${id}"]`).evaluate(node => node.open && node.querySelectorAll('input,button,[role="combobox"]').length > 0); assert.equal(visible[id], true); }
    assert.equal((await state(p)).commands.length, 0); await toggle(p.getByRole('switch', { name: '当前会话自动翻译', exact: true }), false); c.action('disable-current-session-translation'); await p.waitForFunction(() => window.uiHarness.commands.length === 1); const output = await state(p); assert.deepEqual(output.commands[0].command, { kind: 'set_translation_enabled', payload: { enabled: false } }); assert.equal(output.session.translationEnabled, false); c.output({ visible, ...output });
  });
  await check('SC-06-current-session-pause-exact-authority', async c => {
    const p = await general(); c.input(await state(p)); const control = p.getByRole('switch', { name: '新台词自动加入当前会话', exact: true }); assert.equal(await control.isChecked(), true); await toggle(control, false); c.action('pause-current-session'); await p.waitForFunction(() => window.uiHarness.commands.some(e => e.command.kind === 'set_capture_paused'));
    const output = await state(p); assert.equal(output.commands.length, 1); assert.deepEqual(output.commands[0].command, { kind: 'set_capture_paused', payload: { paused: true } }); assert.equal(output.session.paused, true); c.output({ ...output, checked: await control.isChecked() });
  });
  await check('SC-07-pause-no-session-error-and-late-isolation', async c => {
    const p = await general(); const input = await state(p); c.input(input); const control = p.getByRole('switch', { name: '新台词自动加入当前会话', exact: true });
    await p.evaluate(() => window.uiHarness.capturePauseResponses.push({ error: 'SC pause denied' })); await toggle(control, false); await p.getByRole('alert').filter({ hasText: 'SC pause denied' }).waitFor(); c.action('pause-error'); const errorChecked = await control.isChecked(); assert.equal(errorChecked, true);
    await p.evaluate(() => window.uiHarness.capturePauseResponses.push({ holdKey: 'sc-old-pause' })); await toggle(control, false); await p.waitForFunction(() => window.uiHarness.capturePausePending.has('sc-old-pause')); const next = { ...input.session, sessionId: 'sc-new-session', paused: false }; await p.evaluate(next => window.uiHarness.publish({ session: next }), next); await p.evaluate(() => window.uiHarness.releaseCapturePause('sc-old-pause')); await tick(p); c.action('replace-session-before-old-ack'); const afterLate = await state(p); assert.equal(afterLate.session.sessionId, 'sc-new-session'); assert.equal(afterLate.session.paused, false); assert.equal(await control.isChecked(), true);
    await p.evaluate(() => window.uiHarness.publish({ session: null })); await tick(p); c.action('remove-current-session'); const disabled = await control.isDisabled(); assert.equal(disabled, true); c.output({ errorChecked, afterLate, disabled, ...(await state(p)) });
  });
  await check('SC-08-history-always-and-real-navigation', async c => {
    const p = await general(); c.input(await state(p)); const policy = p.locator('[data-history-policy="always"]'); assert.equal(await policy.count(), 1); const switches = await policy.getByRole('switch').count(); assert.equal(switches, 0); await button(p, '查看阅读历史').click(); c.action('open-main-history');
    await p.getByRole('switch', { name: '显示词性标记', exact: true }).waitFor(); const switchGeometry = await switchLabelGeometry(p); c.action('measure-all-described-switch-labels');
    const output = await state(p); assert.ok(output.openedMainPages.includes('history')); assert.equal(output.commands.length, 0); c.output({ ...output, historyPolicy: await policy.getAttribute('data-history-policy'), historySwitches: switches, switchGeometry });
  });
  await check('SC-09-login-query-set-requery-projection', async c => {
    const p = await general(); const input = await state(p); const plan = { setter: login('enabled'), firstRequery: login('requires_approval') }; c.input({ ...input, responsePlan: plan }); const control = p.getByRole('switch', { name: '登录后打开客户端', exact: true }); assert.equal(await control.isChecked(), false);
    await p.evaluate(plan => { window.uiHarness.loginStartupSetResponses.push({ value: plan.setter }); window.uiHarness.loginStartupResponses.push({ value: plan.firstRequery }); }, plan); await toggle(control, true); c.action('explicit-enable-login'); await p.waitForFunction(n => window.uiHarness.loginStartupQueries > n, input.loginQueries); await tick(p);
    const afterSet = { status: await p.locator('[data-login-startup-status]').getAttribute('data-login-startup-status'), text: await p.locator('[data-login-startup-status]').innerText() }; assert.deepEqual(afterSet, { status: 'requires_approval', text: '等待系统批准' }); c.action('observe-independent-requery-overrides-enabled-receipt');
    await button(p, '重新读取登录启动状态').click(); await p.waitForFunction(n => window.uiHarness.loginStartupQueries >= n + 2, input.loginQueries); await tick(p); c.action('explicit-requery-confirms-enabled'); const output = await state(p); assert.deepEqual(output.loginRequests, [true]); assert.equal(await control.isChecked(), true); assert.equal(output.loginFixture.status, 'enabled'); assert.equal(await p.locator('[data-login-startup-status]').getAttribute('data-login-startup-status'), 'enabled'); c.output({ ...output, afterSet, checked: await control.isChecked() });
  });
  await check('SC-10-login-approval-is-not-enabled', async c => {
    const p = await general('reading', { login: login('requires_approval') }); c.input(await state(p)); const checked = await p.getByRole('switch', { name: '登录后打开客户端', exact: true }).isChecked(); const status = await p.locator('[data-login-startup-status]').getAttribute('data-login-startup-status'), statusText = await p.locator('[data-login-startup-status]').innerText(); assert.equal(status, 'requires_approval'); assert.equal(statusText, '等待系统批准'); assert.equal(statusText.includes('已启用'), false); const text = await p.locator('[data-ui-page="general-settings"]').innerText(); assert.match(text, /已登记，尚未生效，等待系统批准/); c.action('read-system-requires-approval'); c.output({ checked, text, status, statusText, ...(await state(p)) });
  });
  await check('SC-11-login-failure-preserves-known-state', async c => {
    const p = await general('reading', { login: login('enabled') }); c.input(await state(p)); const control = p.getByRole('switch', { name: '登录后打开客户端', exact: true });
    await p.evaluate(() => window.uiHarness.loginStartupResponses.push({ error: 'SC login query failed' })); await button(p, '重新读取登录启动状态').click(); await p.getByRole('alert').filter({ hasText: 'SC login query failed' }).waitFor(); const afterRead = await control.isChecked(); assert.equal(afterRead, true); c.action('failed-explicit-query');
    await p.evaluate(() => window.uiHarness.loginStartupSetResponses.push({ error: 'SC login write failed' })); await toggle(control, false); await p.getByRole('alert').filter({ hasText: 'SC login write failed' }).waitFor(); const afterWrite = await control.isChecked(); assert.equal(afterWrite, true); c.action('failed-explicit-disable'); c.output({ afterRead, afterWrite, ...(await state(p)) });
  });
  await check('SC-12-login-busy-and-late-isolation', async c => {
    const p = await general(); c.input(await state(p)); const control = p.getByRole('switch', { name: '登录后打开客户端', exact: true }); await p.evaluate(() => window.uiHarness.loginStartupSetResponses.push({ value: { status: 'enabled', canChange: true, errorCode: null, detail: null }, holdKey: 'sc-login-old' })); await toggle(control, true); await p.waitForFunction(() => window.uiHarness.loginStartupPending.has('sc-login-old')); const busyDisabled = await control.isDisabled(), rereadDisabled = await button(p, '重新读取登录启动状态').isDisabled(); assert.equal(busyDisabled, true); assert.equal(rereadDisabled, true); const duplicateAttempt = await p.getByText('登录后打开客户端', { exact: true }).click({ timeout: 500 }).then(() => ({ blocked: false, error: null }), error => ({ blocked: error.message.includes('element is not enabled'), error: error.message })); assert.equal(duplicateAttempt.blocked, true); assert.equal((await state(p)).loginRequests.length, 1); c.action('hold-enable-and-measure-busy'); c.action('attempt-duplicate-while-busy');
    await button(p, '翻译服务').click(); await p.evaluate(() => { window.uiHarness.loginStartupFixture = { status: 'not_registered', canChange: true, errorCode: null, detail: null }; }); await button(p, '通用').click(); await p.waitForFunction(() => window.uiHarness.loginStartupQueries >= 2); await p.evaluate(() => window.uiHarness.releaseLoginStartup('sc-login-old')); await tick(p); c.action('leave-remount-before-late-old-set'); const checked = await control.isChecked(); assert.equal(checked, false); const output = await state(p); assert.deepEqual(output.loginRequests, [true]); c.output({ busyDisabled, rereadDisabled, duplicateAttempt, checked, ...output });
  });
} catch (error) { errors.push(error.stack ?? String(error)); }
finally { await browser?.close(); if (server) await new Promise(resolve => server.close(resolve)); }
const after = await sourceHashes(); if (JSON.stringify(before) !== JSON.stringify(after)) errors.push('Full source tree changed during execution; rerun frozen sources');
if (externalRequests.length) errors.push('Unexpected external requests');
const result = { schemaVersion: 1, id: 'current-settings-controls-real-react', nonce, executedAt, finishedAt: new Date().toISOString(), status: errors.length === 0 && checks.length === 12 && checks.every(c => c.status === 'passed') ? 'passed' : 'failed', layer: 'real_react_controlled_transport', assertionCount, skippedAssertions: 0, checks, captures, builds, sourceSha256: before, sourceSha256After: after, productionCsp: csp, errors, externalRequests, nativeOrGameVerified: false, productReleaseApproved: false, pixelAcceptance: false, referenceApproval: 'not_granted' };
await fs.writeFile(path.join(out, 'verification.json'), JSON.stringify(result, null, 2) + '\n'); console.log(JSON.stringify({ status: result.status, checks: checks.length, assertionCount, output: out })); if (result.status !== 'passed') process.exitCode = 1;
