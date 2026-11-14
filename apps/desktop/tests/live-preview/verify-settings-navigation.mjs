/** Real settings components and Tauri adapter over a test-owned transport. Never Native acceptance. */
import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { launchLockedBrowser, verifyBrowserIdentity } from '../visual/locked-browser.mjs';
import { startServiceFormRuntime } from '../support/service-form-runtime.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const evidenceRoot = path.join(root, '.build/current-client-execution-20261105/manual-translation-alignment/settings-navigation');
const output = path.join(evidenceRoot, 'run-' + Date.now());
const workspace = path.join(output, 'workspace'), desktop = path.join(workspace, 'apps/desktop');
const owned = [
  'crates/app-core/src/contracts/desktop.rs', 'apps/desktop/src/bridge/contracts.generated.ts',
  'apps/desktop/src-tauri/src/bootstrap.rs', 'apps/desktop/src-tauri/src/ipc/commands.rs',
  'apps/desktop/src-tauri/src/ipc/queries.rs', 'apps/desktop/src-tauri/src/main.rs',
  'apps/desktop/src/bridge/application-port.ts', 'apps/desktop/src/bridge/tauri-port.ts',
  'apps/desktop/src/windows/settings/SettingsWindow.tsx', 'apps/desktop/tests/support/design-port.ts',
  'apps/desktop/tests/live-preview/verify-settings-navigation.mjs', 'crates/app-core/tests/settings_navigation_contract.rs',
];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const results = [], browserErrors = [];
let browser, browserIdentity, server, runtime, origin, serverLog = '', sourceBefore, tracked, fixedBefore;
async function filesUnder(directory) {
  const files = [];
  for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...await filesUnder(file));
    else if (entry.isFile()) files.push(file);
  }
  return files;
}
async function identities() { return Object.fromEntries(await Promise.all(tracked.map(async file => [file, hash(await fs.readFile(path.join(root, file)))]))); }
async function health() {
  return Promise.all([1421, 38677].map(async port => {
    try { const response = await fetch(`http://127.0.0.1:${port}/__luna_live_preview_health`, { signal: AbortSignal.timeout(500) }); return { port, value: await response.json() }; }
    catch { return { port, value: null }; }
  }));
}
async function record(file, value) { await fs.writeFile(path.join(output, file + '.json'), JSON.stringify(value, null, 2) + '\n'); }
async function runCargo(args, label) {
  const command = ['cargo', ...args], child = spawn(command[0], command.slice(1), { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  child.stdout.on('data', chunk => stdout += chunk); child.stderr.on('data', chunk => stderr += chunk);
  const exitCode = await new Promise((resolve, reject) => { child.once('error', reject); child.once('close', resolve); });
  const summaries = [...stdout.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;/g)].map(match => ({ passed: Number(match[1]), failed: Number(match[2]), ignored: Number(match[3]) }));
  const receipt = { command, cwd: root, cache: 'existing workspace target', exitCode, stdout, stderr, summaries };
  await record(label, receipt);
  assert.equal(exitCode, 0, `${label}: ${stderr || stdout}`);
  assert.ok(summaries.reduce((count, row) => count + row.passed, 0) > 0, 'Cargo must actually execute tests');
  assert.ok(summaries.every(row => row.failed === 0 && row.ignored === 0), 'Cargo failures and ignored tests must be zero');
  return { command, summaries };
}
function check(name, run, timeout = 45000) {
  test(name, { timeout }, async () => {
    try { const facts = await run(); results.push({ name, status: 'passed', facts }); }
    catch (error) { results.push({ name, status: 'failed', error: error.stack }); throw error; }
  });
}

before(async () => {
  await fs.mkdir(output, { recursive: true });
  const uiFiles = await Promise.all(['apps/desktop/src', 'apps/desktop/tests/support'].map(directory => filesUnder(path.join(root, directory))));
  tracked = [...new Set([...owned, ...uiFiles.flat().map(file => path.relative(root, file)), 'crates/app-infra/examples/service_form_fixture.rs'])].sort();
  sourceBefore = await identities(); fixedBefore = await health(); await record('source-before', sourceBefore);
  for (const relative of ['apps/desktop/src', 'apps/desktop/tests/support', 'apps/desktop/package.json', 'docs/design/first-product-v1/assets']) {
    const destination = path.join(workspace, relative); await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.cp(path.join(root, relative), destination, { recursive: true });
  }
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir');
  await fs.symlink(path.join(root, 'node_modules'), path.join(workspace, 'node_modules'), 'dir');
  runtime = await startServiceFormRuntime({ root, output });
  const response = await runtime.query({ kind: 'catalog' }); assert.equal(response.ok, true);
  await record('actual-rust-catalog', response.value);
  const harness = `import React,{StrictMode} from 'react';
import {createRoot} from 'react-dom/client';
import {WindowRoot} from './src/app/WindowRoot';
import {ApplicationStore} from './src/bridge/projections/application-store';
import {TauriApplicationPort} from './src/bridge/tauri-port';
import {DesignPort} from './tests/support/design-port';
import './src/ui/reset.css';
document.documentElement.dataset.windowRole='settings';
const options=JSON.parse(new URLSearchParams(location.search).get('options')||'{}');
const catalog=${JSON.stringify(response.value)},port=new DesignPort('model-config');port.catalogFixture=catalog;
const source={kind:'http',source:{revisionId:'navigation-fixture-1',modelId:'navigation-service'}},target={kind:'http',preset:catalog.presets.find(item=>item.key.presetId==='builtin.deepseek').key};
const values={name:'导航测试已有服务',model:'saved-navigation-model',connection_mode:'custom',endpoint:'https://example.invalid/v1',auth_mode:'api_key'};
const calls={forms:[],drafts:[],mutations:[],native:[]};
port.serviceFormFixture=async request=>{calls.forms.push(structuredClone(request));const response=await fetch('/form',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(request)});const value=await response.json();if(!value.ok)throw Error(value.code);return value.value;};
port.serviceFixtures.saved=async query=>({profileGeneration:1,appliedHeadRevision:0,selection:{requestedScope:query.scope,effectiveScope:{kind:'global'},selectionRevision:0,effectiveSelectionRevision:0,main:null,comparisons:[],purposeEnabled:false},rows:query.purpose==='translation'?[{source,logicalResourceId:'navigation-service',displayName:values.name,backend:'http_llm',purpose:'translation',eligibility:{kind:'selectable'},verification:{kind:'untested'}}]:[]});
port.serviceFixtures.draft=async query=>{calls.drafts.push(structuredClone(query));return{profileGeneration:1,draftHeadRevision:0,source,editing:source,target,purpose:'translation',values,authentication:{kind:'existing_pool',poolId:'navigation-pool',poolRevision:1},readonlyReason:null,affectedSharedModelIds:[],fallback:[]};};
for(const method of ['save','select','enabled','inherit','test','cancelTest'])port.serviceFixtures[method]=async value=>{calls.mutations.push({method,value:structuredClone(value)});throw Error('Navigation must not invoke '+method);};
port.serviceFixtures.credential=async request=>{calls.mutations.push({method:'credential',value:structuredClone(request)});return{credentialId:'navigation-credential',credentialVersionId:'navigation-version',poolId:'navigation-pool',poolRevision:2,label:'测试输入',maskedHint:'test-only'};};
let pending=options.pending??null,sequence=0,callbackSequence=0,failReads=options.failRead?1:0,holdQueries=!!options.holdQuery,earlyEvent=options.earlyEvent??null;
const callbacks=new Map(),listeners=new Map(),heldQueries=[];
function deliver(request){for(const handler of listeners.values())callbacks.get(handler)?.({event:'application-settings-page',id:0,payload:structuredClone(request)});}
window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:(_event,id)=>listeners.delete(id)};
window.__TAURI_INTERNALS__={transformCallback:callback=>{const id=++callbackSequence;callbacks.set(id,callback);return id;},invoke:async(command,args)=>{
  calls.native.push({command,args:structuredClone(args)});
  if(command==='plugin:event|listen'){const id=++sequence;listeners.set(id,args.handler);if(earlyEvent){const request=earlyEvent;earlyEvent=null;deliver(request);}return id;}
  if(command==='plugin:event|unlisten'){listeners.delete(args.eventId);return;}
  if(command==='application_settings_page'){if(failReads-- >0)throw Error('controlled pending query failure');const captured=structuredClone(pending);if(holdQueries)return new Promise(resolve=>heldQueries.push({resolve,captured}));return captured;}
  if(command==='application_open_settings_page'){pending={revision:(pending?.revision??0)+1,intent:args.intent};deliver(pending);return;}
  throw Error('Unexpected native command: '+command);
}};
const native=new TauriApplicationPort();port.openSettingsPage=native.openSettingsPage.bind(native);port.onSettingsPageRequested=native.onSettingsPageRequested.bind(native);
const store=new ApplicationStore(port),container=document.getElementById('root');let root=null;
function mount(){root=createRoot(container);const content=<WindowRoot role="settings" store={store}/>;root.render(options.strict?<StrictMode>{content}</StrictMode>:content);}
window.settingsProbe={port,calls,native,deliver,emit:request=>{pending=structuredClone(request);deliver(request);},pending:request=>pending=structuredClone(request),listeners:()=>listeners.size,held:()=>heldQueries.length,releaseQueries:()=>{holdQueries=false;for(const query of heldQueries.splice(0))query.resolve(query.captured);},unmount:()=>{root.unmount();root=null;},mount};
mount();`;
  await fs.writeFile(path.join(desktop, 'navigation-harness.tsx'), harness);
  await fs.writeFile(path.join(desktop, 'index.html'), '<!doctype html><html lang="zh-CN"><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><style>html,body,#root{margin:0;width:100%;height:100%;overflow:hidden}</style><div id="root"></div><script type="module" src="/navigation-harness.tsx"></script></html>');
  const config = path.join(desktop, 'navigation-vite.config.mjs');
  await fs.writeFile(config, `import react from '@vitejs/plugin-react';export default{root:${JSON.stringify(desktop)},plugins:[react()],cacheDir:${JSON.stringify(path.join(output, 'vite-cache'))},server:{host:'127.0.0.1',proxy:{'/form':${JSON.stringify(runtime.origin)}}}};`);
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = `http://127.0.0.1:${port}`;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', config, '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', chunk => serverLog += chunk); server.stderr.on('data', chunk => serverLog += chunk);
  for (let index = 0; index < 200; index++) {
    try { if ((await fetch(origin)).ok) break; } catch {}
    if (index === 199 || server.exitCode !== null) throw Error('Isolated server failed: ' + serverLog);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());
}, { timeout: 600000 });

after(async () => {
  if (browser) { await verifyBrowserIdentity(browserIdentity); await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('close', resolve)); }
  if (runtime) await runtime.close();
  const sourceAfter = await identities(), fixedAfter = await health();
  await record('results', { kind: 'settings-navigation-real-components-and-rust-tests', sourceBefore, sourceAfter, fixedBefore, fixedAfter, browser: browserIdentity, results, browserErrors, native: 'not_run', product_completion: 'incomplete', limits: ['Tauri transport is explicitly test controlled; native show/focus and packaged role enforcement are not run.', 'Actual Rust catalogue and form projection; no profile, account or provider requests.'] });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  assert.deepEqual(sourceAfter, sourceBefore, 'Sources must remain stable throughout verification');
  assert.deepEqual(fixedAfter, fixedBefore, 'Fixed preview processes must remain unchanged');
  assert.deepEqual(browserErrors, []);
  console.log('Settings navigation evidence: ' + output);
});

async function open(options = {}) {
  const page = await browser.newPage({ viewport: { width: 1000, height: 720 }, deviceScaleFactor: 1 }); page.setDefaultTimeout(8000);
  page.on('pageerror', error => browserErrors.push(error.message));
  await page.goto(`${origin}/?options=${encodeURIComponent(JSON.stringify(options))}`);
  await page.locator('[data-guided-service-editor] [data-service-field="model"] input').waitFor();
  await page.waitForFunction(() => document.querySelector('[data-guided-service-editor]')?.getAttribute('aria-busy') === 'false');
  return page;
}
const nav = page => page.getByRole('navigation', { name: '设置分类', exact: true });
const category = (page, label) => nav(page).getByRole('button', { name: label, exact: true });
const model = page => page.locator('[data-guided-service-editor] [data-service-field="model"] input');
const secret = page => page.locator('[data-guided-service-editor] [data-service-field="api_key"] input');
const leave = page => page.getByRole('dialog', { name: '离开前放弃未保存的编辑？', exact: true });
async function selected(page, label) { await page.waitForFunction(label => document.querySelector('nav[aria-label="设置分类"] [aria-current="page"]')?.textContent === label, label); }
async function settle(page) { await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function noMutations(page) { assert.deepEqual(await page.evaluate(() => ({ mutations: window.settingsProbe.calls.mutations, commands: window.settingsProbe.port.commands })), { mutations: [], commands: [] }); }
async function screenshot(page, name) { await page.evaluate(() => document.fonts.ready); await page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' }); }

check('SETTINGS-NAV-RUST-DTO-actual-contract-tests', () => runCargo(['test', '--locked', '--offline', '-p', 'app-core', '--test', 'settings_navigation_contract'], 'rust-dto-tests'), 600000);
check('SETTINGS-NAV-RUST-REQUEST-actual-helper-tests', () => runCargo(['test', '--locked', '--offline', '-p', 'luna-reader', 'settings_navigation'], 'rust-request-tests'), 600000);

check('SETTINGS-NAV-01-initial-pending-and-typed-native-call', async () => {
  const page = await open({ pending: { revision: 7, intent: 'translation' } }); await selected(page, '翻译服务');
  await category(page, '通用').click(); await selected(page, '通用');
  await page.evaluate(() => window.settingsProbe.deliver({ revision: 7, intent: 'translation' }));
  await settle(page); await selected(page, '通用');
  await page.evaluate(() => window.settingsProbe.native.openSettingsPage('translation')); await selected(page, '翻译服务');
  const calls = await page.evaluate(() => window.settingsProbe.calls.native);
  assert.deepEqual(calls.find(call => call.command === 'application_open_settings_page').args, { intent: 'translation' });
  assert.equal(calls.some(call => call.command === 'application_window'), false); await noMutations(page); await page.close();
});
check('SETTINGS-NAV-02-listen-before-query-and-early-event', async () => {
  const page = await open({ pending: { revision: 1, intent: 'translation' }, earlyEvent: { revision: 2, intent: 'translation' } });
  const calls = await page.evaluate(() => window.settingsProbe.calls.native.map(call => call.command));
  assert.deepEqual(calls.slice(0, 2), ['plugin:event|listen', 'application_settings_page']);
  await category(page, '通用').click(); await selected(page, '通用');
  await page.evaluate(() => window.settingsProbe.deliver({ revision: 1, intent: 'translation' })); await settle(page); await selected(page, '通用');
  await page.evaluate(() => window.settingsProbe.deliver({ revision: 2, intent: 'translation' })); await settle(page); await selected(page, '通用');
  await page.evaluate(() => window.settingsProbe.emit({ revision: 3, intent: 'translation' })); await selected(page, '翻译服务');
  await noMutations(page); await page.close();
});
check('SETTINGS-NAV-03-late-query-old-and-duplicate-revisions', async () => {
  const page = await open({ pending: { revision: 1, intent: 'translation' }, holdQuery: true });
  await page.waitForFunction(() => window.settingsProbe.held() === 1);
  await category(page, '通用').click(); await page.evaluate(() => window.settingsProbe.emit({ revision: 2, intent: 'translation' })); await selected(page, '翻译服务');
  await category(page, '通用').click(); await page.evaluate(() => window.settingsProbe.releaseQueries());
  await settle(page); await selected(page, '通用');
  await page.evaluate(() => { window.settingsProbe.deliver({ revision: 1, intent: 'translation' }); window.settingsProbe.deliver({ revision: 2, intent: 'translation' }); });
  await settle(page); await selected(page, '通用'); await noMutations(page); await page.close();
});
check('SETTINGS-NAV-04-existing-category-clears-navigation-search-and-keeps-preference-draft', async () => {
  const page = await open(); await category(page, '显示与快捷键').click();
  const font = page.getByRole('spinbutton', { name: '原文字号（pt）', exact: true }); await font.fill('31');
  await category(page, '通用').click(); await page.getByRole('searchbox', { name: '搜索设置', exact: true }).fill('网络');
  await page.evaluate(() => window.settingsProbe.native.openSettingsPage('translation')); await selected(page, '翻译服务');
  assert.equal(await page.getByRole('searchbox', { name: '搜索设置', exact: true }).inputValue(), '');
  await category(page, '显示与快捷键').click(); assert.equal(await font.inputValue(), '31');
  await noMutations(page); await screenshot(page, 'preference-draft-retained'); await page.close();
});
check('SETTINGS-NAV-05-dirty-same-page-keeps-editor-and-secret', async () => {
  const page = await open(); await secret(page).fill('synthetic-navigation-secret'); await model(page).fill('unsaved-model');
  const drafts = await page.evaluate(() => { window.navigationEditorNode = document.querySelector('[data-guided-service-editor]'); return window.settingsProbe.calls.drafts.length; });
  await page.evaluate(async () => { await window.settingsProbe.native.openSettingsPage('translation'); await window.settingsProbe.native.openSettingsPage('translation'); });
  await selected(page, '翻译服务'); assert.equal(await leave(page).count(), 0);
  assert.equal(await model(page).inputValue(), 'unsaved-model'); assert.equal(await secret(page).inputValue(), 'synthetic-navigation-secret');
  assert.equal(await page.evaluate(() => document.querySelector('[data-guided-service-editor]') === window.navigationEditorNode), true);
  assert.equal(await page.evaluate(() => window.settingsProbe.calls.drafts.length), drafts); await noMutations(page);
  await page.locator('[data-guided-service-page]>footer').getByRole('button', { name: '保存配置', exact: true }).click();
  await page.waitForFunction(() => window.settingsProbe.calls.mutations.some(call => call.method === 'credential'));
  assert.equal(await page.evaluate(() => window.settingsProbe.calls.mutations.find(call => call.method === 'credential').value.material.key), 'synthetic-navigation-secret', 'The retained secret must also remain in the actual editor owner');
  await page.close();
});
check('SETTINGS-NAV-06-pending-leave-is-cancelled-without-reset', async () => {
  const page = await open(); await model(page).fill('pending-leave-draft'); await secret(page).fill('synthetic-pending-secret');
  await category(page, '通用').click(); await leave(page).waitFor();
  await page.evaluate(() => window.settingsProbe.native.openSettingsPage('translation')); await leave(page).waitFor({ state: 'hidden' });
  assert.equal(await model(page).inputValue(), 'pending-leave-draft'); assert.equal(await secret(page).inputValue(), 'synthetic-pending-secret');
  await noMutations(page); await screenshot(page, 'pending-leave-cancelled'); await page.close();
});
check('SETTINGS-NAV-07-real-leave-keeps-existing-confirm-and-explicit-discard', async () => {
  const page = await open(); await model(page).fill('explicit-discard-draft'); await secret(page).fill('synthetic-discard-secret');
  await category(page, '通用').click(); await leave(page).waitFor(); await page.getByRole('button', { name: '继续编辑', exact: true }).click();
  assert.equal(await model(page).inputValue(), 'explicit-discard-draft'); assert.equal(await secret(page).inputValue(), 'synthetic-discard-secret');
  await category(page, '通用').click(); await leave(page).waitFor(); await page.getByRole('button', { name: '放弃并离开', exact: true }).click(); await selected(page, '通用');
  await category(page, '翻译服务').click(); await model(page).waitFor(); assert.equal(await model(page).inputValue(), 'saved-navigation-model'); assert.equal(await secret(page).inputValue(), '');
  await noMutations(page); await page.close();
});
check('SETTINGS-NAV-08-query-failure-cleans-listener-and-retry-preserves-editing', async () => {
  const page = await open({ failRead: true }); await page.getByText(/设置导航连接失败：controlled pending query failure/).waitFor();
  await page.waitForFunction(() => window.settingsProbe.listeners() === 0);
  await model(page).fill('draft-during-query-failure'); await page.evaluate(() => window.settingsProbe.pending({ revision: 1, intent: 'translation' }));
  await page.getByRole('button', { name: '重新连接设置导航', exact: true }).click();
  await page.waitForFunction(() => window.settingsProbe.listeners() === 1); await page.getByText(/设置导航连接失败/).waitFor({ state: 'hidden' });
  assert.equal(await model(page).inputValue(), 'draft-during-query-failure');
  await category(page, '通用').click(); await leave(page).waitFor();
  await page.evaluate(() => window.settingsProbe.native.openSettingsPage('translation')); await leave(page).waitFor({ state: 'hidden' });
  assert.equal(await model(page).inputValue(), 'draft-during-query-failure'); await noMutations(page); await page.close();
});
check('SETTINGS-NAV-09-strict-mode-and-late-unsubscribe-do-not-leak-listeners', async () => {
  const page = await open({ strict: true, holdQuery: true, pending: { revision: 1, intent: 'translation' } });
  await page.waitForFunction(() => window.settingsProbe.held() >= 2);
  await page.evaluate(() => { window.settingsProbe.unmount(); window.settingsProbe.releaseQueries(); });
  await page.waitForFunction(() => window.settingsProbe.listeners() === 0);
  await page.evaluate(() => window.settingsProbe.mount()); await model(page).waitFor(); await page.waitForFunction(() => window.settingsProbe.listeners() === 1);
  await page.evaluate(() => window.settingsProbe.native.openSettingsPage('translation')); await selected(page, '翻译服务');
  await page.evaluate(() => window.settingsProbe.unmount()); await page.waitForFunction(() => window.settingsProbe.listeners() === 0);
  await noMutations(page); await page.close();
});
