/** User-approved General V3 on real production components. Transport is memory-only;
 * it never grants Native registration, persisted DB, approved pixels or release. */
import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:net';
import { launchLockedBrowser, verifyBrowserIdentity } from '../visual/locked-browser.mjs';
import { generalSourceCaptureInputs, captureGeneralSourceQualification } from '../visual/general-source-capture-provider.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const evidenceRoot = path.join(root, '.build/current-client-execution-20261107/general-settings-structure/followup-20261108');
const output = path.join(evidenceRoot, 'qa/run-' + Date.now());
const desktop = path.join(output, 'workspace/apps/desktop');
const promotedRelative = 'docs/design/first-product-v1/candidates/general-settings-structure-20261107';
const beforeRelative = 'apps/desktop/tests/support/general-settings-before';
const promoted = path.join(root, promotedRelative);
// Historical relative layout is reconstructed in this run's private output. It is never an input path.
const capturedLayout = '.build/current-client-execution-20261105/layout-refinement-implementation-20261107/settings-structure-v3';
let original;
const pinnedRecords = {
  [promotedRelative+'/manifest.json']: 'efd9e1b52c908a60ddfaa3f4e66c4efb6a8cba494d744db67ea010574005398a',
  [promotedRelative+'/captured/capture-manifest.json']: '8bf2b8a3eb51a055c4c9150bb280f8eb5bb6eb9c2795b430a0056bc08490bcc0',
  [promotedRelative+'/captured/capture-freeze.json']: 'a2f576edd5b114a3b8312d70b0634b93af05909c97461deee8478317d8404703',
  [beforeRelative+'/manifest.json']: '385add6352920d4c2e9ad8eaa978036372948b121c11f245dbdbbdc14f2931de',
};
const require = createRequire(path.join(root, 'package.json'));
const { PNG } = require('pngjs');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const ownedProduction = ['PreferencesScreen.tsx', 'GeneralSettings.tsx', 'styles.module.css'].map(name => 'apps/desktop/src/features/preferences/' + name);
const thisTest = 'apps/desktop/tests/live-preview/verify-general-settings-structure.mjs';
const results = [], browserErrors = [], caseNames = [];
const expectedCaseCount = 18;
let browser, identity, server, origin, serverLog = '', sourceBefore, healthBefore, nativeBefore;
let browserVerified = false;
async function fileMap(directory, prefix) {
  const map = {};
  for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name), relative = prefix + '/' + entry.name;
    if (entry.isDirectory()) Object.assign(map, await fileMap(absolute, relative));
    else if (entry.isFile()) map[relative] = hash(await fs.readFile(absolute));
    else throw new Error('Symlink is not a valid private test input: ' + relative);
  }
  return map;
}
const designLeaves = ['index.html', 'settings.html', 'settings.css', 'settings.js', 'README.md'];
async function verifyPermanentInputs(base) {
  for (const [relative, expected] of Object.entries(pinnedRecords)) assert.equal(hash(await fs.readFile(path.join(base, relative))), expected, 'Pinned permanent record changed: ' + relative);
  const beforeManifest = JSON.parse(await fs.readFile(path.join(base, beforeRelative, 'manifest.json'), 'utf8'));
  assert.equal(beforeManifest.production_import_permitted, false); assert.equal(beforeManifest.source_tree_sha256, '5093178435fbb6492d5a5197d89574412520e1149fa117eb666eb10d5b2adbce');
  assert.deepEqual(Object.keys(beforeManifest.files).sort(), ['GeneralSettings.tsx.capture', 'PreferencesScreen.tsx.capture', 'styles.module.css.capture']);
  for (const [name, entry] of Object.entries(beforeManifest.files)) {
    assert.equal(entry.original_path, 'apps/desktop/src/features/preferences/' + name.replace(/\.capture$/, ''));
    const bytes = await fs.readFile(path.join(base, beforeRelative, name)); assert.equal(bytes.length, entry.bytes); assert.equal(hash(bytes), entry.sha256);
  }
  const manifest = JSON.parse(await fs.readFile(path.join(base, promotedRelative, 'manifest.json'), 'utf8'));
  const capture = JSON.parse(await fs.readFile(path.join(base, promotedRelative, 'captured/capture-freeze.json'), 'utf8'));
  for (const [relative, entry] of Object.entries(manifest.sources)) { assert.ok(relative.startsWith(promotedRelative+'/') && !relative.includes('..')); const bytes = await fs.readFile(path.join(base, relative)); assert.equal(bytes.length, entry.bytes); assert.equal(hash(bytes), entry.sha256); }
  for (const name of designLeaves) { const bytes = await fs.readFile(path.join(base, promotedRelative, 'captured', name)), entry = capture.design_files[name]; assert.equal(bytes.length, entry.bytes); assert.equal(hash(bytes), entry.sha256); }
  for (const entry of capture.reused_dependencies) { assert.ok(entry.path.startsWith('docs/design/first-product-v1/') && !entry.path.includes('..')); const bytes = await fs.readFile(path.join(base, entry.path)); assert.equal(bytes.length, entry.bytes); assert.equal(hash(bytes), entry.sha256); }
  return {beforeManifest,manifest,capture};
}
async function reconstructCaptured(base, destination) {
  const {capture} = await verifyPermanentInputs(base); const directory = path.join(destination, capturedLayout); await fs.mkdir(directory,{recursive:true});
  for (const name of designLeaves) await fs.copyFile(path.join(base,promotedRelative,'captured',name),path.join(directory,name));
  await fs.copyFile(path.join(base,promotedRelative,'reported-native-before.png'),path.join(directory,'reported-native-before.png'));
  for (const entry of capture.reused_dependencies) { const target=path.join(destination,entry.path);await fs.mkdir(path.dirname(target),{recursive:true});await fs.copyFile(path.join(base,entry.path),target); }
  return directory;
}
async function identities(base = root) {
  const map = await fileMap(path.join(base, 'apps/desktop/src'), 'apps/desktop/src');
  const paths = [...generalSourceCaptureInputs(), thisTest, 'apps/desktop/tests/support/design-port.ts', 'apps/desktop/tests/visual/locked-browser.mjs', 'apps/desktop/package.json', 'apps/desktop/package-lock.json', 'package.json', 'package-lock.json', 'crates/app-core/src/contracts/desktop.rs', 'docs/design/first-product-v1/material-tokens.css', 'docs/design/first-product-v1/icons.js'];
  for (const name of [...designLeaves,'reported-native-before.png','manifest.json']) paths.push(promotedRelative+'/'+name);
  for (const name of [...designLeaves,'capture-manifest.json','capture-freeze.json']) paths.push(promotedRelative+'/captured/'+name);
  for (const name of ['PreferencesScreen.tsx.capture','GeneralSettings.tsx.capture','styles.module.css.capture','manifest.json']) paths.push(beforeRelative+'/'+name);
  for (const relative of paths) { assert.ok(!relative.startsWith('.build/') && !relative.includes('..') && !path.isAbsolute(relative));map[relative] = hash(await fs.readFile(path.join(base, relative))); }
  return map;
}
async function fixedHealth() {
  try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(1000) })).json(); }
  catch { return null; }
}
function nativePid() {
  return execFileSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).split('\n').filter(line => line.trim().endsWith('/Users/tanghehui/Applications/Luna.app/Contents/MacOS/luna-reader')).map(line => Number(line.trim().split(/\s+/, 1)[0]));
}
async function record(name, value) { await fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n'); }
function check(name, run) {
  assert.ok(!caseNames.includes(name)); caseNames.push(name);
  test(name, { timeout: 120000 }, async () => {
    try { const facts = await run(); results.push({ name, status: 'passed', facts }); }
    catch (error) { results.push({ name, status: 'failed', error: error.stack }); throw error; }
  });
}
before(async () => {
  await fs.mkdir(desktop, { recursive: true });
  sourceBefore = await identities(); await verifyPermanentInputs(root); original = await reconstructCaptured(root, path.join(output,'original-layout')); healthBefore = await fixedHealth(); nativeBefore = nativePid();
  await record('source-before', sourceBefore);
  for (const relative of ['src', 'tests/support', 'package.json']) await fs.cp(path.join(root, 'apps/desktop', relative), path.join(desktop, relative), { recursive: true });
  await fs.cp(path.join(desktop, 'src'), path.join(desktop, 'before-src'), { recursive: true });
  for (const relative of ownedProduction) await fs.copyFile(path.join(root,beforeRelative,path.basename(relative)+'.capture'),path.join(desktop,'before-src',relative.split('/src/')[1]));
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir');
  await fs.symlink(path.join(root, 'node_modules'), path.join(output, 'workspace/node_modules'), 'dir');
  await fs.writeFile(path.join(desktop, 'general-harness.tsx'), `import React from 'react';
import {createRoot} from 'react-dom/client';
import {ApplicationStore} from './src/bridge/projections/application-store';
import {DesignPort} from './tests/support/design-port';
import type {MainPageIntent,MainPageRequest,ScopedPreferences} from './src/bridge/contracts.generated';
import './src/ui/reset.css';
const options=JSON.parse(new URLSearchParams(location.search).get('options')||'{}');
document.documentElement.dataset.windowRole=options.role==='main'?'main':'settings';
const {WindowRoot}=options.before?await import('./before-src/app/WindowRoot'):await import('./src/app/WindowRoot');
class GeneralSettingsPort extends DesignPort {
  navigationResponses:{intent?:MainPageIntent,error?:string,holdKey?:string}[]=[];
  navigationPending=new Map<string,()=>void>();navigationAttempts:MainPageIntent[]=[];
  consumerListeners=new Set<(request:MainPageRequest)=>void>();consumerArchive:((request:MainPageRequest)=>void)[]=[];consumerCleanupCount=0;consumerRegistrationPending=new Map<string,()=>void>();consumerRegistrationHolds:string[]=options.mainSubscriptionHold?[options.mainSubscriptionHold]:[];
  async onMainPageRequested(listener:(request:MainPageRequest)=>void){this.consumerListeners.add(listener);this.consumerArchive.push(listener);const unsubscribe=await super.onMainPageRequested(listener);const hold=this.consumerRegistrationHolds.shift();if(hold)await new Promise<void>(resolve=>this.consumerRegistrationPending.set(hold,resolve));return()=>{this.consumerListeners.delete(listener);this.consumerCleanupCount++;unsubscribe();};}
  emitMainRequest(request:MainPageRequest){for(const listener of this.consumerListeners)listener(structuredClone(request));}
  emitArchivedMainRequest(index:number,request:MainPageRequest){const listener=this.consumerArchive[index];if(!listener)throw new Error('No controlled archived consumer');listener(structuredClone(request));}
  releaseConsumerRegistration(key:string){const release=this.consumerRegistrationPending.get(key);if(!release)throw new Error('Controlled subscription registration hold missing');this.consumerRegistrationPending.delete(key);release();}

  async openMainPage(intent:MainPageIntent){this.navigationAttempts.push(intent);const index=this.navigationResponses.findIndex(item=>!item.intent||item.intent===intent),response=index<0?undefined:this.navigationResponses.splice(index,1)[0];if(response?.holdKey)await new Promise<void>(resolve=>this.navigationPending.set(response.holdKey!,resolve));if(response?.error)throw new Error(response.error);return super.openMainPage(intent);}
  releaseNavigation(key:string){const release=this.navigationPending.get(key);if(!release)throw new Error('Controlled navigation hold missing');this.navigationPending.delete(key);release();}
  async preferencesScope(scopeKey:string):Promise<ScopedPreferences>{const view=await super.preferencesScope(scopeKey);if(view.inheritedDraft)view.inheritedDraft={...structuredClone(view.global),scopeKey,overrides:{}};return view;}
}
const port=new GeneralSettingsPort('reading');
port.state.preferences={...port.state.preferences,scopeKey:'global',theme:options.theme??'dark',segmentationEnabled:true,showRuby:true,showPos:false};
port.state.games=[{gameId:'general-structure-game',displayName:'サクラノ詩',originalTitle:null,sourceLanguage:'ja',coverAssetRef:null,rowRevision:1,target:{environmentKind:'unresolved',environmentKey:'',bottleLocator:null,launchPath:null,canonicalExePath:null,targetPathKey:null,workingDirectory:null,arguments:[],architecture:null,verificationState:'unresolved'}}];
if(options.maskedScope){
 const global={...structuredClone(port.state.preferences),scopeKey:'global'};port.scopedPreferenceSaves.set('global',{saved:global,headRevision:1,preferenceRevisionId:'masked-global-1'});
 port.state.games.push({...structuredClone(port.state.games[0]),gameId:'general-active-game-b',displayName:'受控活动游戏B'});
 const scopeKey='game:general-active-game-b',saved={...structuredClone(global),scopeKey,showRuby:false,showPos:true,overrides:{showRuby:true,showPos:true}};port.scopedPreferenceSaves.set(scopeKey,{saved,headRevision:1,preferenceRevisionId:'masked-active-b-1'});
 port.state.preferences=port.controlledScope(scopeKey).effective;
 if(port.state.session)port.state.session={...port.state.session,gameId:'general-active-game-b'};
 if(port.state.current)port.state.current={...port.state.current,gameId:'general-active-game-b',sourceSummary:port.state.current.sourceSummary?{...port.state.current.sourceSummary,gameId:'general-active-game-b'}:null};
}
if(options.session===null)port.state.session=null;
if(options.sessionMode==='empty'){port.state.session=null;port.state.current=null;port.state.history={units:[],nextCursor:null};}
if(options.sessionMode==='stopped'&&port.state.session)port.state.session={...port.state.session,state:'stopped'};
port.loginStartupFixture=options.startup??{status:'not_found',canChange:false,errorCode:'service_not_found',detail:'受控原始诊断：系统未找到应用服务；不能由此判定重装可以修复。'};
if(options.startupResponse)port.loginStartupResponses.push(options.startupResponse);
if(options.scopeResponse)port.scopedPreferenceResponses.push(options.scopeResponse);
port.serviceFixtures.saved=async query=>({profileGeneration:port.state.profileGeneration,appliedHeadRevision:port.state.configuration.appliedHeadRevision,selection:{requestedScope:query.scope,effectiveScope:{kind:'global'},selectionRevision:0,effectiveSelectionRevision:0,main:null,comparisons:[],purposeEnabled:false},rows:[]});
const store=new ApplicationStore(port),host=createRoot(document.getElementById('root')),role=options.role==='main'?'main':'settings';let epoch=0;const render=()=>host.render(options.paired?<div key={epoch} style={{display:'grid',gridTemplateColumns:'1000px 1000px',height:'100%',width:2000}}><div data-harness-main style={{minWidth:0,height:'100%'}}><WindowRoot role="main" store={store}/></div><div data-harness-settings style={{minWidth:0,height:'100%'}}><WindowRoot role="settings" store={store}/></div></div>:<WindowRoot key={epoch} role={role} store={store}/>);
window.generalSettings={port,store,options,transport:'private-memory-only-not-native-or-database',remount(){epoch++;render();}};render();`);
  await fs.writeFile(path.join(desktop, 'index.html'), '<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><style>html,body,#root{margin:0;width:100%;height:100%;overflow:hidden}body{background:#161827}</style><div id="root"></div><script type="module" src="/general-harness.tsx"></script></html>');
  const config = path.join(desktop, 'general-vite.config.mjs');
  await fs.writeFile(config, `import react from '@vitejs/plugin-react';export default{root:${JSON.stringify(desktop)},plugins:[react()],cacheDir:${JSON.stringify(path.join(output, 'vite-cache'))},server:{host:'127.0.0.1',fs:{allow:[${JSON.stringify(path.join(output, 'workspace'))},${JSON.stringify(root)}]}}};`);
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve)); const reservedPort = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = 'http://127.0.0.1:' + reservedPort;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', config, '--port', String(reservedPort), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', chunk => serverLog += chunk); server.stderr.on('data', chunk => serverLog += chunk);
  for (let index = 0; index < 150; index++) { try { if ((await fetch(origin)).ok) break; } catch {} if (index === 149 || server.exitCode !== null) throw Error('Private Vite failed: ' + serverLog); await new Promise(resolve => setTimeout(resolve, 100)); }
  ({ browser, identity } = await launchLockedBrowser());
}, { timeout: 120000 });
after(async () => {
  const integrityErrors = [];
  if (browser) { try { await verifyBrowserIdentity(identity); browserVerified = true; } catch (error) { integrityErrors.push(error.stack); } await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('close', resolve)); }
  const sourceAfter = await identities(), healthAfter = await fixedHealth(), nativeAfter = nativePid();
  const sourceStable = JSON.stringify(sourceAfter) === JSON.stringify(sourceBefore), previewStable = JSON.stringify(healthBefore) === JSON.stringify(healthAfter), nativeStable = JSON.stringify(nativeBefore) === JSON.stringify(nativeAfter);
  const completeCaseSet = caseNames.length === expectedCaseCount && results.length === expectedCaseCount && results.every((item, index) => item.name === caseNames[index]);
  const executionStatus = completeCaseSet && results.every(item => item.status === 'passed') && sourceStable && previewStable && nativeStable && browserVerified && !browserErrors.length && !integrityErrors.length ? 'passed' : 'failed';
  await record('results', { kind: 'real-general-settings-v3-controlled-port', sourceCaptureEnvironment: { reducedMotion: 'reduce; actual matchMedia asserted in every source page', css: 'approved source media rule only; no style injection', comparison: 'every RGBA pixel; threshold0; no mask; no image-selection retry' }, execution_status: executionStatus, expectedCaseCount, completeCaseSet, results, sourceBefore, sourceAfter, sourceStable, healthBefore, healthAfter, previewStable, nativeBefore, nativeAfter, nativeStable, browser: identity, browserVerified, browserErrors, integrityErrors, native: 'not_run', database: 'not_run', product_completion: 'incomplete', approved_pixel_acceptance: false, limits: ['All scope/startup/navigation effects here are explicit private memory transport responses, not persisted SQL or macOS ServiceManagement.', 'The 8 design relocation comparisons prove dependency URL relocation only; production screenshots and structured layout/behavior facts are separate.', 'Old spacing/capture-control layout cases retain their historical sources and runs; only the exact approved new General contract supersedes those view assumptions.'] });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog); console.log('General settings V3 evidence: ' + output);
  assert.equal(executionStatus, 'passed', 'Incomplete/failed cases, source drift or browser/preview/native drift cannot accept this suite');
});
async function settle(page) { await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function screenshot(page, name) { await page.evaluate(() => document.fonts.ready); await settle(page); await page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' }); }
const general = page => page.locator('[data-ui-page="general-settings"]');
const control = (page, label) => general(page).getByRole('switch', { name: label, exact: true });
const save = page => general(page).getByRole('button', { name: '保存阅读偏好', exact: true });
const discard = page => general(page).getByRole('button', { name: '放弃修改', exact: true });
const scope = page => general(page).getByRole('combobox', { name: '阅读偏好作用范围', exact: true });
async function open(options = {}) {
  const page = await browser.newPage({ viewport: { width: options.width ?? 1000, height: options.height ?? 720 }, deviceScaleFactor: 1 });
  page.setDefaultTimeout(9000); page.on('pageerror', error => browserErrors.push(error.message));
  await page.goto(origin + '/?options=' + encodeURIComponent(JSON.stringify(options)));
  if(options.role==='main'){await page.getByRole('navigation', {name:'主导航'}).getByRole('button',{name:'当前会话',exact:true}).click();await page.getByRole('button',{name:'暂停自动输入',exact:true}).waitFor();if(options.zoom)await page.evaluate(zoom=>document.documentElement.style.zoom=String(zoom),options.zoom);return page;}
  await page.getByRole('navigation', { name: '设置分类' }).getByRole('button', { name: '通用', exact: true }).click();
  await general(page).waitFor();
  if (!options.scopeResponse?.holdKey && !options.scopeResponse?.error) await save(page).waitFor();
  if (options.zoom) { await page.evaluate(zoom => document.documentElement.style.zoom = String(zoom), options.zoom); await settle(page); }
  return page;
}
async function setToggle(page, label, value, keyboard = false) {
  const input = control(page, label); await input.scrollIntoViewIfNeeded();
  if (await input.isChecked() !== value) { if (keyboard) { await input.focus(); await page.keyboard.press('Space'); } else await input.locator('xpath=ancestor::label').click(); }
  assert.equal(await input.isChecked(), value);
}
async function chooseScope(page, label) {
  await scope(page).scrollIntoViewIfNeeded(); await scope(page).click(); const option = page.getByRole('option', { name: label, exact: true });
  const facts = await option.evaluate(node => { const r = node.getBoundingClientRect(); const hit = document.elementsFromPoint(r.x + r.width / 2, r.y + r.height / 2); return { rect: { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom }, hit: hit.some(item => item === node || node.contains(item)), viewport: { width: innerWidth, height: innerHeight } }; });
  assert.equal(facts.hit, true, 'The pointer target must be topmost, including at 200%'); assert.ok(facts.rect.x >= 0 && facts.rect.right <= facts.viewport.width + 1 && facts.rect.y >= 0 && facts.rect.bottom <= facts.viewport.height + 1); await option.click(); await settle(page); return facts;
}
async function reach(page, locator) {
  await locator.scrollIntoViewIfNeeded(); const facts = await locator.evaluate(node => { const r = node.getBoundingClientRect(); const scroller = node.closest('[data-ui-scroll="preferences"]'); const p = scroller?.getBoundingClientRect(); return { rect: { x: r.x, y: r.y, right: r.right, bottom: r.bottom }, viewport: { width: innerWidth, height: innerHeight }, centerInsideScroll: !p || r.x + r.width / 2 >= p.x && r.x + r.width / 2 <= p.right && r.y + r.height / 2 >= p.y && r.y + r.height / 2 <= p.bottom }; });
  assert.ok(facts.rect.x >= 0 && facts.rect.y >= 0 && facts.rect.right <= facts.viewport.width + 1 && facts.rect.bottom <= facts.viewport.height + 1); assert.equal(facts.centerInsideScroll, true); return facts;
}
const mainConsumer = page => page.locator('[data-harness-main]');
async function mainConsumerFacts(page){return { selected:await mainConsumer(page).getByRole('navigation',{name:'主导航'}).locator('[aria-current=page]').textContent(), sessionVisible:await mainConsumer(page).locator('[data-session-layout]').isVisible(), targetChooserCount:await page.getByRole('dialog',{name:'选择正在运行的游戏',exact:true}).count() };}
async function expectMainPage(page,expected){const names={history:'历史记录',session:'当前会话',capture:'游戏库'};await page.waitForFunction(name=>document.querySelector('[data-harness-main] nav[aria-label="主导航"] [aria-current=page]')?.textContent?.trim()===name,names[expected],{timeout:2500});if(expected==='session')assert.equal(await mainConsumer(page).locator('[data-session-layout]').isVisible(),true);}
async function stateFacts(page) { return page.evaluate(() => { const { port } = window.generalSettings; return { session: port.state.session, commands: port.commands.map(item => item.command), saved: [...port.scopedPreferenceSaves], startupWrites: port.loginStartupRequests, nav: port.mainPageRequests, navAttempts: port.navigationAttempts }; }); }
async function exactPixels(left, right, name) {
  const a = PNG.sync.read(left), b = PNG.sync.read(right); assert.equal(a.width, b.width); assert.equal(a.height, b.height); let changed = 0; const delta = new PNG({ width: a.width, height: a.height });
  for (let i = 0; i < a.data.length; i += 4) { const different = [0, 1, 2, 3].some(offset => a.data[i + offset] !== b.data[i + offset]); if (different) changed++; delta.data[i] = different ? 255 : 0; delta.data[i + 1] = 0; delta.data[i + 2] = 0; delta.data[i + 3] = 255; }
  await fs.writeFile(path.join(output, name + '-raw-diff.png'), PNG.sync.write(delta)); return { differingPixels: changed, totalPixels: a.width * a.height, threshold: 0, mask: null, includeEveryRgbaPixel: true };
}
async function sourcePage(directory, options) {
  const page = await browser.newPage({ viewport: { width: options.width, height: options.height }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
  page.setDefaultTimeout(9000); page.on('pageerror', error => browserErrors.push(error.message));
  await page.goto(pathToFileURL(path.join(directory, 'settings.html')).href + `?theme=${options.theme}&zoom=${options.zoom}`); await page.waitForFunction(() => Boolean(window.SettingsStructureCandidate)); await page.evaluate(() => document.fonts.ready); await settle(page);
  const environment = await page.evaluate(() => ({ prefersReducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches, devicePixelRatio, viewport: { width: innerWidth, height: innerHeight }, fonts: document.fonts.status, runningAnimations: document.getAnimations().filter(animation => animation.playState === 'running' || animation.playState === 'pending').length })); assert.equal(environment.prefersReducedMotion, true); assert.equal(environment.fonts, 'loaded'); assert.equal(environment.runningAnimations, 0); await record(`source-environment-${hash(Buffer.from(directory)).slice(0, 12)}-${options.width}-${options.theme}-${options.zoom}`, environment); return page;
}
async function sourceStyle(page) { return page.evaluate(() => { const css = n => getComputedStyle(n); const row = document.querySelector('.lexical-card .toggle-row'), label = row.querySelector('label'), description = row.querySelector('p'), track = row.querySelector('[role=switch]'); return { heading: css(document.querySelector('h1')).fontSize, headingWeight: css(document.querySelector('h1')).fontWeight, family: css(document.body).fontFamily, groupHeading: css(document.querySelector('.group h2')).fontSize, label: css(label).fontSize, description: css(description).fontSize, rowPadding: css(row).padding, trackWidth: css(track).width, trackHeight: css(track).height, groupMargin: css(document.querySelector('.group')).marginBottom }; }); }
async function actualStyle(page) {
  return general(page).evaluate(node => {
    const zoom = Number.parseFloat(getComputedStyle(document.documentElement).zoom) || 1, rect = item => { const r = item.getBoundingClientRect(); return { x: r.x / zoom, y: r.y / zoom, width: r.width / zoom, height: r.height / zoom, right: r.right / zoom, bottom: r.bottom / zoom }; }, css = item => getComputedStyle(item);
    const lexical = [...node.querySelectorAll('section')].find(section => section.querySelector('h2')?.textContent === '分词与词典');
    if (!lexical) throw Error('The approved lexical group is absent');
    const toggles = [...lexical.querySelectorAll('[data-switch-label]')].map(label => { const row = label.closest('label'), track = label.nextElementSibling, r = rect(row), l = rect(label), t = rect(track); return { label: css(label.querySelector('strong')).fontSize, description: css(label.querySelector('small')).fontSize, rowPadding: css(row).padding, trackWidth: css(track).width, trackHeight: css(track).height, leftInset: l.x - r.x, rightInset: r.right - t.right, overlap: l.right > t.x, row: r }; });
    const groupHeadings = [...node.querySelectorAll('section>h2')], local = groupHeadings.find(item => item.textContent === '本机偏好')?.parentElement, footer = node.querySelector('footer'), scroller = node.querySelector('[data-ui-scroll="preferences"]');
    return { heading: css(node.querySelector('h1')).fontSize, headingWeight: css(node.querySelector('h1')).fontWeight, family: css(node).fontFamily, scopedFamilies: [node.querySelector('h1'),lexical.querySelector('h2'),lexical.querySelector('strong'),lexical.querySelector('small'),footer].map(item=>css(item).fontFamily), groupHeading: css(lexical.querySelector('h2')).fontSize, groupMargin: css(lexical).marginBottom, toggles, groups: groupHeadings.map(item => item.textContent), local: rect(local), footer: rect(footer), scroller: rect(scroller), horizontalOverflow: node.scrollWidth > node.clientWidth || scroller.scrollWidth > scroller.clientWidth + 1, documentOverflow: document.documentElement.scrollWidth > innerWidth, headingLeft: rect(node.querySelector('h1')).x, lexicalLeft: rect(lexical.querySelector('h2+div')).x };
  });
}

check('GENERAL-01-approved-design-promotion-is-byte-scoped-and-pixel-identical', async () => {
  const originalHtml = await fs.readFile(path.join(original, 'settings.html'), 'utf8'), promotedHtml = await fs.readFile(path.join(promoted, 'settings.html'), 'utf8');
  assert.equal(promotedHtml, originalHtml.replace('../../../../docs/design/first-product-v1/material-tokens.css', '../../material-tokens.css').replace('../../../../docs/design/first-product-v1/icons.js', '../../icons.js'));
  for (const name of ['index.html', 'settings.css', 'settings.js', 'README.md', 'reported-native-before.png']) assert.deepEqual(await fs.readFile(path.join(original, name)), await fs.readFile(path.join(promoted, name)));
  const facts = [];
  for (const width of [1000, 760]) for (const theme of ['dark', 'light']) for (const zoom of [1, 2]) {
    const options = { width, height: width === 1000 ? 720 : 560, theme, zoom }, left = await sourcePage(original, options), right = await sourcePage(promoted, options), name = `promotion-${width}-${theme}-${zoom}`;
    const a = await left.screenshot({ path: path.join(output, name + '-captured.png'), animations: 'disabled' }), b = await right.screenshot({ path: path.join(output, name + '-promoted.png'), animations: 'disabled' });
    // Original Chromium canvases stay as an unqualified-observer diagnostic.
    // Byte/8-canvas/whole-canvas/zero obligations use independently calibrated WK.
    const chromiumDiagnostic = await exactPixels(a, b, name + '-chromium-unqualified');
    const qualified = await captureGeneralSourceQualification({ output: path.join(output, name + '-qualified-wk'), ...options });
    const diff = await exactPixels(qualified.left, qualified.right, name); assert.equal(diff.differingPixels, 0); facts.push({ options, diff, qualifiedObserver: qualified.receipt, chromiumDiagnostic: { qualification: 'not_qualified', ...chromiumDiagnostic } }); await left.close(); await right.close();
  }
  const pages = [await sourcePage(original, { width: 1000, height: 720, theme: 'dark', zoom: 1 }), await sourcePage(promoted, { width: 1000, height: 720, theme: 'dark', zoom: 1 })]; const buffers = [];
  for (const [index, page] of pages.entries()) { await page.locator('#scope-button').click(); await page.getByRole('option', { name: '仅此游戏 · サクラノ詩', exact: true }).click(); await page.getByRole('switch', { name: '显示日语注音', exact: true }).click(); await page.getByRole('button', { name: '保存阅读偏好', exact: true }).click(); await page.waitForFunction(() => document.querySelector('#toast').hidden); await page.evaluate(() => document.fonts.ready); await settle(page); const presentation = await page.evaluate(() => ({ toastHidden: document.querySelector('#toast').hidden, prefersReducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches, runningAnimations: document.getAnimations().filter(animation => animation.playState === 'running' || animation.playState === 'pending').map(animation => ({playState:animation.playState,currentTime:animation.currentTime})), reading: window.SettingsStructureCandidate.state, knobs: [...document.querySelectorAll('.switch>span')].map(node => ({left:getComputedStyle(node).left,transform:getComputedStyle(node).transform})) })); assert.equal(presentation.toastHidden, true); assert.equal(presentation.prefersReducedMotion, true); assert.deepEqual(presentation.runningAnimations, []); await record(`promotion-game-${index}-presentation`, presentation); buffers.push(await page.screenshot({ path: path.join(output, `promotion-game-${index}.png`), animations: 'disabled' })); }
  const chromiumGameDiagnostic = await exactPixels(buffers[0], buffers[1], 'promotion-game-chromium-unqualified');
  const qualifiedGame = await captureGeneralSourceQualification({ output: path.join(output, 'promotion-game-qualified-wk'), width: 1000, height: 720, theme: 'dark', zoom: 1, savedGame: true });
  const game = await exactPixels(qualifiedGame.left, qualifiedGame.right, 'promotion-game'); assert.equal(game.differingPixels, 0); for (const page of pages) await page.close(); return { combinations: facts, savedGameOverride: game, qualifiedGameObserver: qualifiedGame.receipt, chromiumGameDiagnostic: { qualification: 'not_qualified', ...chromiumGameDiagnostic }, motionPreference: 'reduce on both source canvases before interaction; existing approved media rule', approvalScope: 'Dependency URL relocation only; not production pixels' };
});

check('GENERAL-02-real-components-match-approved-rhythm-and-all-controls-remain-reachable', async () => {
  const facts = [];
  for (const width of [1000, 760]) for (const theme of ['dark', 'light']) for (const zoom of [1, 2]) {
    const options = { width, height: width === 1000 ? 720 : 560, theme, zoom }, reference = await sourcePage(promoted, options), expected = await sourceStyle(reference), page = await open(options);
    await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); const actual = await actualStyle(page);
    await record(`typography-${width}-${theme}-${zoom}`, { expected, actual }); await screenshot(page, `typography-${width}-${theme}-${zoom}`); assert.equal(actual.headingWeight, expected.headingWeight); assert.equal(actual.family, expected.family); assert.ok(actual.scopedFamilies.every(family=>family===expected.family), 'Only the approved General content inherits the source family');
    assert.deepEqual(actual.groups, ['分词与词典', '阅读记录', '本机偏好']); assert.equal(actual.heading, expected.heading); assert.equal(actual.groupHeading, expected.groupHeading); assert.equal(actual.groupMargin, expected.groupMargin); assert.equal(actual.toggles.length, 3); assert.equal(actual.horizontalOverflow, false); assert.equal(actual.documentOverflow, false); assert.ok(Math.abs(actual.headingLeft - actual.lexicalLeft) < 1.1);
    for (const toggle of actual.toggles) { for (const key of ['label', 'description', 'rowPadding', 'trackWidth', 'trackHeight']) assert.equal(toggle[key], expected[key]); assert.ok(toggle.leftInset >= 13.9 && toggle.rightInset >= 13.9); assert.equal(toggle.overlap, false); }
    if (width === 1000 && zoom === 1) assert.ok(actual.local.bottom <= actual.footer.y + 1, 'Default complete local row must be above the fixed footer');
    assert.equal(await control(page, '新台词自动加入当前会话').count(), 0); assert.equal(await general(page).getByRole('heading', { name: '产品范围', exact: true }).count(), 0); assert.equal(await general(page).getByRole('switch', { name: /历史/ }).count(), 0);
    const name = `client-${width}-${theme}-${zoom}`; await screenshot(page, name + '-top'); const sourceBuffer = await reference.screenshot({ path: path.join(output, name + '-source.png'), animations: 'disabled' }), clientBuffer = await page.screenshot({ animations: 'disabled' }); const rawDifference = await exactPixels(sourceBuffer, clientBuffer, name + '-source-client');
    const pointer = []; for (const label of ['显示词性标记', '显示日语注音', '可点击日语分词']) { pointer.push(await reach(page, control(page, label).locator('xpath=ancestor::label'))); await setToggle(page, label, label === '显示词性标记'); }
    assert.equal(await control(page, '显示词性标记').isDisabled(), true); assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await control(page, '显示日语注音').isDisabled(), false); assert.equal(await scope(page).isDisabled(), true);
    await reach(page, save(page)); await save(page).click(); await page.waitForFunction(() => window.generalSettings.port.scopedPreferenceSaves.has('global')); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor();
    const saved = await page.evaluate(() => window.generalSettings.port.scopedPreferenceSaves.get('global').saved); assert.deepEqual([saved.segmentationEnabled, saved.showRuby, saved.showPos], [false, false, true]);
    await setToggle(page, '可点击日语分词', true, true); assert.equal(await control(page, '显示词性标记').isChecked(), true); await reach(page, discard(page)); await discard(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '可点击日语分词').isChecked(), false);
    const optionsPointer = [await chooseScope(page, '仅此游戏 · サクラノ詩'), await chooseScope(page, '全局默认')]; await screenshot(page, name + '-scoped'); facts.push({ options, expected, actual, pointer, optionsPointer, sourceClientRawDifference: rawDifference, sourceClientPixelAccepted: false }); await page.close(); await reference.close();
  }
  return facts;
});

check('GENERAL-03-admitted-before-image-proves-exactly-the-approved-structure-change', async () => {
  const before = await open({ before: true }), after = await open();
  assert.equal(await general(before).getByRole('heading', { name: '阅读行为', exact: true }).count(), 1); assert.equal(await control(before, '新台词自动加入当前会话').count(), 1); assert.equal(await general(before).getByRole('heading', { name: '产品范围', exact: true }).count(), 1);
  assert.equal(await general(after).getByRole('heading', { name: '阅读行为', exact: true }).count(), 0); assert.equal(await control(after, '新台词自动加入当前会话').count(), 0); assert.equal(await general(after).getByRole('heading', { name: '产品范围', exact: true }).count(), 0);
  assert.equal(await general(after).getByRole('button', { name: '阅读历史', exact: true }).count(), 1); assert.equal(await general(after).getByRole('button', { name: '当前会话', exact: true }).count(), 1); await screenshot(before, 'before-structure'); await screenshot(after, 'after-structure'); const actual = await actualStyle(after); await before.close(); await after.close(); return { actual, beforeInputs: ownedProduction.map(relative => beforeRelative+'/'+path.basename(relative)+'.capture') };
});

check('GENERAL-04-per-field-game-overrides-restore-only-draft-and-inherit-later-global-values', async () => {
  const page = await open(), game = 'game:general-structure-game';
  await chooseScope(page, '仅此游戏 · サクラノ詩'); await general(page).getByText('当前来源：继承全局', { exact: true }).waitFor(); assert.equal(await general(page).getByRole('button', { name: '恢复继承全局', exact: true }).isDisabled(), true);
  await setToggle(page, '显示日语注音', false); assert.equal(await scope(page).isDisabled(), true); assert.equal(await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.has(key), game), false); await save(page).click(); await page.waitForFunction(key => window.generalSettings.port.scopedPreferenceSaves.has(key), game); await general(page).getByText(/当前来源：游戏覆盖 · 1项/).waitFor();
  let saved = await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key).saved, game); assert.deepEqual(saved.overrides, { showRuby: true }); assert.equal(saved.showRuby, false);
  await general(page).getByText('查看已保存的覆盖项', { exact: true }).click(); const savedDetails = general(page).getByText('查看已保存的覆盖项', { exact: true }).locator('..'); assert.equal(await savedDetails.evaluate(node => node.open), true); assert.equal(await savedDetails.isVisible(), true); assert.match(await savedDetails.innerText(), /日语注音/);
  await chooseScope(page, '全局默认'); await setToggle(page, '显示词性标记', true); await save(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); await chooseScope(page, '仅此游戏 · サクラノ詩'); assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await control(page, '显示日语注音').isChecked(), false);
  const beforeRestore = await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key), game); await general(page).getByRole('button', { name: '恢复继承全局', exact: true }).click(); assert.equal(await general(page).getByRole('button', { name: '恢复继承全局', exact: true }).isDisabled(), true); assert.equal(await control(page, '显示日语注音').isChecked(), true); await general(page).getByText(/草稿：恢复继承全局，保存后生效/).waitFor(); assert.deepEqual(await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key), game), beforeRestore); assert.equal(await scope(page).isDisabled(), true); await screenshot(page, 'game-restored-unsaved');
  await discard(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.deepEqual(await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key), game), beforeRestore);
  await general(page).getByRole('button', { name: '恢复继承全局', exact: true }).click(); await save(page).click(); await page.waitForFunction(key => Object.keys(window.generalSettings.port.scopedPreferenceSaves.get(key).saved.overrides).length === 0, game); await general(page).getByText('当前来源：继承全局', { exact: true }).waitFor();
  await chooseScope(page, '全局默认'); await setToggle(page, '显示日语注音', false); await save(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); await chooseScope(page, '仅此游戏 · サクラノ詩'); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await general(page).getByRole('button', { name: '恢复继承全局', exact: true }).isDisabled(), true);
  // Fresh real component editor with already-saved non-General aliases. A
  // General edit must name every retained override without losing its payload.
  const aliases = await open();
  await aliases.evaluate(key => { const p=window.generalSettings.port; const saved={...structuredClone(p.state.preferences),scopeKey:key,showRuby:false,overrides:{'hud.compact':true,'hud.alwaysOnTop':true,'reading.segmentationEnabled':true}};p.scopedPreferenceSaves.set(key,{saved,headRevision:1,preferenceRevisionId:'existing-alias-overrides-1'}); },game);
  await chooseScope(aliases,'仅此游戏 · サクラノ詩'); await general(aliases).getByText('当前来源：游戏覆盖 · 3项',{exact:true}).waitFor();
  const aliasBefore=await aliases.evaluate(key=>window.generalSettings.port.scopedPreferenceSaves.get(key),game);
  assert.equal(await control(aliases,'显示日语注音').isChecked(),true, 'Unmasked stored ruby must inherit global rather than stale saved value');
  await setToggle(aliases,'显示日语注音',false);
  const hint=general(aliases).getByText(/^草稿：覆盖/);
  const hintText=await hint.innerText();for(const label of ['阅读密度','置顶','日语分词','日语注音'])assert.ok(hintText.includes(label),label);
  assert.doesNotMatch(hintText,/、、|覆盖、|、，|undefined|null/);
  assert.deepEqual(await aliases.evaluate(key=>window.generalSettings.port.scopedPreferenceSaves.get(key),game),aliasBefore);
  await discard(aliases).click();await general(aliases).getByText('阅读偏好已保存',{exact:false}).waitFor();
  assert.deepEqual(await aliases.evaluate(key=>window.generalSettings.port.scopedPreferenceSaves.get(key),game),aliasBefore);assert.equal(await control(aliases,'显示日语注音').isChecked(),true);
  await setToggle(aliases,'显示日语注音',false);await save(aliases).click();await aliases.waitForFunction(key=>window.generalSettings.port.scopedPreferenceSaves.get(key)?.headRevision===2,game);
  const aliasAfter=await aliases.evaluate(key=>window.generalSettings.port.scopedPreferenceSaves.get(key),game),aliasFacts=await stateFacts(aliases);
  assert.equal(aliasAfter.saved.showRuby,false);assert.deepEqual(aliasAfter.saved.overrides,{...aliasBefore.saved.overrides,showRuby:true});
  const aliasPayload=aliasFacts.commands.filter(command=>command.kind==='save_scoped_preferences').at(-1).payload.preferences;
  for(const [key,value]of Object.entries(aliasBefore.saved.overrides))assert.equal(aliasPayload.overrides[key],value,key);
  for(const [key,value]of Object.entries(aliasBefore.saved))if(!['showRuby','overrides'].includes(key))assert.deepEqual(aliasPayload[key],value,key);
  await screenshot(aliases,'game-mixed-alias-overrides');await aliases.close();
  const facts = await stateFacts(page); const gameSaves = facts.commands.filter(command => command.kind === 'save_scoped_preferences' && command.payload.preferences.scopeKey === game); assert.deepEqual(gameSaves.map(command => command.payload.expectedHeadRevision), [null, 1]); assert.equal(facts.commands.some(command => command.kind === 'set_capture_paused'), false); await screenshot(page, 'game-inherits-later-global'); await page.close(); return {...facts,mixedAliasOverride:{before:aliasBefore,after:aliasAfter,hintText,commands:aliasFacts.commands}};
});

check('GENERAL-05-save-failure-and-head-conflict-preserve-draft-and-retry-current-authority', async () => {
  const page = await open(), game = 'game:general-structure-game'; await chooseScope(page, '仅此游戏 · サクラノ詩'); await setToggle(page, '显示日语注音', false);
  await page.evaluate(() => window.generalSettings.port.scopedPreferenceSaveResponses.push({ error: '受控保存失败' })); await save(page).click(); await general(page).getByRole('alert').filter({ hasText: '受控保存失败' }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await save(page).isEnabled(), true); assert.equal(await scope(page).isDisabled(), true); assert.equal(await page.evaluate(() => window.generalSettings.port.scopedPreferenceSaves.size), 0);
  await page.evaluate(() => window.generalSettings.port.preferenceSaveHoldKeys.push('head-conflict')); await save(page).click(); await page.waitForFunction(() => window.generalSettings.port.preferenceSavePending.has('head-conflict'));
  await page.evaluate(key => { const p = window.generalSettings.port, view = p.controlledScope(key); p.scopedPreferenceSaves.set(key, { saved: { ...structuredClone(view.effective), scopeKey: key, showPos: true, overrides: { showPos: true } }, headRevision: 42, preferenceRevisionId: 'external-controlled-head' }); p.releasePreferenceSave('head-conflict'); }, game);
  await general(page).getByRole('alert').filter({ hasText: 'preference_head_conflict' }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await save(page).isEnabled(), true); assert.equal(await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key).headRevision, game), 42); await screenshot(page, 'cas-conflict-draft-retained');
  await discard(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), true); assert.equal(await control(page, '显示词性标记').isChecked(), true); await setToggle(page, '显示日语注音', false); await save(page).click(); await page.waitForFunction(key => window.generalSettings.port.scopedPreferenceSaves.get(key).headRevision === 43, game);
  const facts = await stateFacts(page); const last = facts.commands.filter(command => command.kind === 'save_scoped_preferences').at(-1); assert.equal(last.payload.expectedHeadRevision, 42); assert.deepEqual(last.payload.preferences.overrides, { showPos: true, showRuby: true }); await page.close(); return facts;
});

check('GENERAL-06-scope-read-error-and-late-window-response-never-replace-a-new-draft', async () => {
  const page = await open({ scopeResponse: { scopeKey: 'global', error: '受控读取失败' } }); await general(page).getByRole('alert').filter({ hasText: '受控读取失败' }).waitFor(); assert.equal(await save(page).isDisabled(), true);
  await page.evaluate(() => { const p = window.generalSettings.port, old = p.controlledScope('global'); old.effective.showRuby = false; p.scopedPreferenceResponses.push({ scopeKey: 'global', value: old, holdKey: 'scope-old-window' }); }); await general(page).getByRole('button', { name: '重新读取偏好来源', exact: true }).click(); await page.waitForFunction(() => window.generalSettings.port.scopedPreferencePending.has('scope-old-window'));
  await page.evaluate(() => window.generalSettings.remount()); await page.getByRole('navigation', { name: '设置分类' }).getByRole('button', { name: '通用', exact: true }).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), true); await page.evaluate(() => window.generalSettings.port.releasePreferenceScope('scope-old-window')); await settle(page); assert.equal(await control(page, '显示日语注音').isChecked(), true);
  await setToggle(page, '显示日语注音', false); const reads = await page.evaluate(() => window.generalSettings.port.scopedPreferenceQueries.length); await page.evaluate(() => { const p = window.generalSettings.port; p.publish({ preferences: { ...p.state.preferences, sourceFontPt: 19 } }); }); await settle(page); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await scope(page).isDisabled(), true); assert.equal(await page.evaluate(() => window.generalSettings.port.scopedPreferenceQueries.length), reads); const facts = await stateFacts(page); assert.equal(facts.commands.length, 0); await screenshot(page, 'late-scope-window-ignored'); await page.close(); return facts;
});

check('GENERAL-07-typed-history-session-navigation-has-pending-errors-and-no-write-side-effects', async () => {
  const page = await open({ paired:true,width:2000,height:720 }); await setToggle(page, '显示日语注音', false); const before = await stateFacts(page);
  await page.evaluate(() => window.generalSettings.port.navigationResponses.push({ intent: 'history', holdKey: 'history-navigation' })); await general(page).getByRole('button', { name: '阅读历史', exact: true }).click(); await page.waitForFunction(() => window.generalSettings.port.navigationPending.has('history-navigation')); assert.equal(await general(page).getByRole('button', { name: '当前会话', exact: true }).isDisabled(), true); assert.equal(await general(page).getByRole('button', { name: '阅读历史', exact: true }).isDisabled(), true); assert.equal(await control(page, '显示日语注音').isChecked(), false);
  await page.evaluate(() => window.generalSettings.port.releaseNavigation('history-navigation')); await page.waitForFunction(() => window.generalSettings.port.mainPageRequests.length === 1); assert.equal(await page.evaluate(() => window.generalSettings.port.mainPageRequests[0].intent), 'history'); await expectMainPage(page,'history');
  await page.evaluate(() => window.generalSettings.port.navigationResponses.push({ intent: 'session', error: '受控会话导航失败' })); await general(page).getByRole('button', { name: '当前会话', exact: true }).click(); await general(page).getByRole('alert').filter({ hasText: '受控会话导航失败' }).waitFor({ timeout: 2500 }); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await scope(page).isDisabled(), true); await general(page).getByRole('button', { name: '当前会话', exact: true }).click(); await page.waitForFunction(() => window.generalSettings.port.mainPageRequests.length === 2);
  await expectMainPage(page,'session'); assert.equal(await page.getByRole('dialog',{name:'选择正在运行的游戏',exact:true}).count(),0); const facts = await stateFacts(page); assert.deepEqual(facts.nav.map(item => item.intent), ['history', 'session']); assert.deepEqual(facts.navAttempts, ['history', 'session', 'session']); assert.deepEqual(facts.session, before.session); assert.deepEqual(facts.saved, before.saved); assert.deepEqual(facts.commands, []); assert.deepEqual(facts.startupWrites, []); await screenshot(page, 'typed-main-navigation'); await page.close(); return facts;
});

check('GENERAL-08-startup-not-found-read-failure-and-reread-remain-independent-from-preference-save', async () => {
  const page = await open(); const login = control(page, '登录后打开 Luna'); await login.waitFor(); assert.equal(await login.isDisabled(), true); assert.equal(await login.isChecked(), false); assert.equal(await general(page).getByText(/受控原始诊断/).count(), 0); await general(page).getByRole('button', { name: '查看原因', exact: true }).click(); await general(page).getByText(/受控原始诊断/).waitFor(); await general(page).getByText(/service_not_found/).waitFor();
  await page.evaluate(() => window.generalSettings.port.loginStartupResponses.push({ error: '受控系统读取失败', holdKey: 'startup-read' })); await general(page).getByRole('button', { name: '重新读取状态', exact: true }).click(); await page.waitForFunction(() => window.generalSettings.port.loginStartupPending.has('startup-read')); assert.equal(await login.isChecked(), false); await setToggle(page, '显示日语注音', false); await save(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await login.isDisabled(), true);
  await page.evaluate(() => window.generalSettings.port.releaseLoginStartup('startup-read')); await general(page).getByRole('alert').filter({ hasText: '受控系统读取失败' }).waitFor(); await general(page).getByText('当前状态待确认', { exact: true }).waitFor(); assert.equal(await login.isChecked(), false); assert.equal(await control(page, '显示日语注音').isChecked(), false);
  await page.evaluate(() => { const p = window.generalSettings.port; p.loginStartupFixture = { status: 'enabled', canChange: true, errorCode: null, detail: '受控系统随后确认已启用' }; }); await general(page).getByRole('button', { name: '重新读取状态', exact: true }).click(); await general(page).locator('[data-login-startup-status="enabled"]').waitFor(); assert.equal(await login.isChecked(), true); const facts = await stateFacts(page); assert.deepEqual(facts.startupWrites, []); assert.equal(facts.commands.filter(command => command.kind === 'save_scoped_preferences').length, 1); assert.equal(facts.commands.some(command => command.kind === 'set_capture_paused'), false); await screenshot(page, 'startup-reread-and-saved-preferences'); await page.close(); return facts;
});

check('GENERAL-09-startup-write-receipt-is-followed-by-authoritative-readback-and-failure-preserves-state', async () => {
  const page = await open({ startup: { status: 'not_registered', canChange: true, errorCode: null, detail: '受控系统未启用' } }), login = control(page, '登录后打开 Luna'); await login.waitFor(); await page.waitForFunction(() => window.generalSettings.port.loginStartupQueries > 0); await login.locator('xpath=ancestor::label').waitFor();
  await page.evaluate(() => { const p = window.generalSettings.port; p.loginStartupSetResponses.push({ value: { status: 'requires_approval', canChange: true, errorCode: null, detail: '受控写回执待批准' }, holdKey: 'startup-write' }); p.loginStartupResponses.push({ value: { status: 'not_registered', canChange: true, errorCode: null, detail: '受控系统仍未启用' }, holdKey: 'startup-confirm' }); }); await login.locator('xpath=ancestor::label').click(); await page.waitForFunction(() => window.generalSettings.port.loginStartupPending.has('startup-write')); assert.equal(await login.isDisabled(), true); assert.equal(await login.isChecked(), false, 'Write intention before native receipt is not success');
  await page.evaluate(() => window.generalSettings.port.releaseLoginStartup('startup-write')); await page.waitForFunction(() => window.generalSettings.port.loginStartupPending.has('startup-confirm')); assert.equal(await login.isDisabled(), true); await general(page).getByText(/正在核对/).waitFor(); await page.evaluate(() => window.generalSettings.port.releaseLoginStartup('startup-confirm')); await general(page).locator('[data-login-startup-status="not_registered"]').waitFor(); await page.waitForFunction(node => !node.disabled, await login.elementHandle()); assert.equal(await login.isChecked(), false); assert.equal(await login.isEnabled(), true);
  await page.evaluate(() => { const p = window.generalSettings.port; const value = { status: 'enabled', canChange: true, errorCode: null, detail: '受控系统已启用' }; p.loginStartupSetResponses.push({ value }); p.loginStartupResponses.push({ value }); }); await login.locator('xpath=ancestor::label').click(); await general(page).locator('[data-login-startup-status="enabled"]').waitFor(); await page.waitForFunction(node => !node.disabled, await login.elementHandle()); assert.equal(await login.isChecked(), true);
  await page.evaluate(() => window.generalSettings.port.loginStartupSetResponses.push({ error: '受控系统禁止取消' })); await login.locator('xpath=ancestor::label').click(); await general(page).getByText('当前状态待确认', { exact: true }).waitFor(); assert.equal(await login.isChecked(), true); await general(page).getByRole('button', { name: '查看原因', exact: true }).click(); await general(page).getByRole('alert').filter({ hasText: '受控系统禁止取消' }).waitFor(); const facts = await stateFacts(page); assert.deepEqual(facts.startupWrites, [true, true, false]); assert.deepEqual(facts.commands, []); assert.deepEqual(facts.saved, []); await screenshot(page, 'startup-confirmation-and-write-failure'); await page.close(); return facts;
});

check('GENERAL-10-unmounted-startup-response-cannot-replace-the-next-window-lifetime', async () => {
  const page = await open({ startupResponse: { value: { status: 'not_found', canChange: false, errorCode: 'old_not_found', detail: '受控旧窗口回执' }, holdKey: 'startup-old-window' } }); await page.waitForFunction(() => window.generalSettings.port.loginStartupPending.has('startup-old-window'));
  await page.evaluate(() => { const p = window.generalSettings.port; p.loginStartupFixture = { status: 'enabled', canChange: true, errorCode: null, detail: '受控新窗口确认已启用' }; window.generalSettings.remount(); }); await page.getByRole('navigation', { name: '设置分类' }).getByRole('button', { name: '通用', exact: true }).click(); await general(page).locator('[data-login-startup-status="enabled"]').waitFor(); assert.equal(await control(page, '登录后打开 Luna').isChecked(), true); await page.evaluate(() => window.generalSettings.port.releaseLoginStartup('startup-old-window')); await settle(page); assert.equal(await control(page, '登录后打开 Luna').isChecked(), true); assert.equal(await general(page).locator('[data-login-startup-status="not_found"]').count(), 0); const facts = await stateFacts(page); assert.deepEqual(facts.startupWrites, []); assert.deepEqual(facts.commands, []); await page.close(); return facts;
});

check('GENERAL-11-preference-save-freezes-payload-and-does-not-clobber-newer-user-edits', async () => {
  const page = await open(); await setToggle(page, '显示日语注音', false); await page.evaluate(() => window.generalSettings.port.preferenceSaveHoldKeys.push('preference-write')); await save(page).click(); await page.waitForFunction(() => window.generalSettings.port.preferenceSavePending.has('preference-write')); assert.equal(await scope(page).isDisabled(), true); await setToggle(page, '显示词性标记', true); await page.evaluate(() => window.generalSettings.port.releasePreferenceSave('preference-write')); await save(page).waitFor(); await page.waitForFunction(() => window.generalSettings.port.scopedPreferenceSaves.has('global')); await page.waitForFunction(() => !document.querySelector('[data-ui-page="general-settings"] button[aria-busy=true]'));
  assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await save(page).isEnabled(), true); assert.equal(await page.evaluate(() => window.generalSettings.port.scopedPreferenceSaves.get('global').saved.showPos), false); await save(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor();
  const facts = await stateFacts(page), writes = facts.commands.filter(command => command.kind === 'save_scoped_preferences'); assert.deepEqual(writes.map(command => [command.payload.preferences.showRuby, command.payload.preferences.showPos, command.payload.expectedHeadRevision]), [[false, false, null], [false, true, 1]]); await screenshot(page, 'save-generation-retains-newer-edit'); await page.close(); return facts;
});

check('GENERAL-12-other-preference-page-and-settings-navigation-remain-unchanged', async () => {
  const before = await open({ before: true }), after = await open(); const buffers = [];
  for (const [index, page] of [before, after].entries()) { await page.getByRole('navigation', { name: '设置分类' }).getByRole('button', { name: '显示与快捷键', exact: true }).click(); await page.getByText('原生磨砂材质', { exact: true }).waitFor(); await page.evaluate(() => document.fonts.ready); await settle(page); buffers.push(await page.screenshot({ path: path.join(output, `unaffected-display-${index}.png`), animations: 'disabled' })); }
  const diff = await exactPixels(buffers[0], buffers[1], 'unaffected-display'); assert.equal(diff.differingPixels, 0, 'General-only CSS must not restyle the existing display page'); await before.close();
  const nav = after.getByRole('navigation', { name: '设置分类' }); await nav.getByRole('button', { name: '通用', exact: true }).click(); assert.equal(await general(after).getByRole('heading', { name: '通用', exact: true }).count(), 1); await after.getByRole('searchbox', { name: '搜索设置', exact: true }).fill('通用'); assert.equal(await nav.getByRole('button').count(), 1); await after.getByRole('searchbox', { name: '搜索设置', exact: true }).fill(''); assert.equal(await nav.getByRole('button').count(), 10); await nav.getByRole('button', { name: 'OCR引擎', exact: true }).click(); await after.getByText('本期窗口预览独立于 OCR', { exact: false }).waitFor(); const facts = await stateFacts(after); assert.deepEqual(facts.startupWrites, []); assert.deepEqual(facts.commands, []); await after.close(); return { displayPixelDifference: diff, navigationFacts: facts };
});

check('GENERAL-13-session-receive-control-lives-in-the-real-main-page-with-failure-pause-and-resume', async () => {
  const page = await open({ role: 'main' }); const before = await stateFacts(page); const originalText = await page.evaluate(() => window.generalSettings.port.state.current.sourceText), preferences = await page.evaluate(() => structuredClone(window.generalSettings.port.state.preferences));
  await page.evaluate(() => window.generalSettings.port.capturePauseResponses.push({ error: '受控接收控制失败' })); await page.getByRole('button', { name: '暂停自动输入', exact: true }).click(); await page.getByRole('alert').filter({ hasText: '受控接收控制失败' }).waitFor(); assert.equal(await page.evaluate(() => window.generalSettings.port.state.session.paused), false);
  await page.getByRole('button', { name: '暂停自动输入', exact: true }).click(); await page.getByRole('button', { name: '继续自动输入', exact: true }).waitFor(); assert.equal(await page.evaluate(() => window.generalSettings.port.state.session.paused), true); assert.equal(await page.evaluate(() => window.generalSettings.port.state.current.sourceText), originalText);
  await page.getByRole('button', { name: '继续自动输入', exact: true }).click(); await page.getByRole('button', { name: '暂停自动输入', exact: true }).waitFor(); assert.equal(await page.evaluate(() => window.generalSettings.port.state.session.paused), false); const facts = await stateFacts(page); assert.deepEqual(facts.commands.map(command => [command.kind, command.payload.paused]), [['set_capture_paused', true], ['set_capture_paused', true], ['set_capture_paused', false]]); assert.equal(facts.session.translationEnabled, before.session.translationEnabled); assert.equal(facts.session.sessionId, before.session.sessionId); assert.deepEqual(await page.evaluate(() => window.generalSettings.port.state.preferences), preferences); assert.deepEqual(facts.saved, []); assert.deepEqual(facts.startupWrites, []); await screenshot(page, 'session-receive-control-failure-pause-resume'); await page.close(); return { facts, controlLocation: 'existing real Main current session; not General' };
});

check('GENERAL-14-clean-nonactive-game-scope-refreshes-on-external-global-change-but-dirty-draft-is-preserved', async () => {
  const page = await open(), game = 'game:general-structure-game'; await chooseScope(page, '仅此游戏 · サクラノ詩');
  const beforeQueries = await page.evaluate(key => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length, game); assert.equal(await page.evaluate(() => window.generalSettings.port.state.preferences.scopeKey), 'global'); assert.equal(await control(page, '显示词性标记').isChecked(), false);
  await page.evaluate(() => { const p = window.generalSettings.port, saved = { ...structuredClone(p.controlledScope('global').effective), scopeKey: 'global', showPos: true }; p.scopedPreferenceSaves.set('global', { saved, headRevision: 1, preferenceRevisionId: 'external-global-1' }); p.publish({ preferences: saved }); });
  let refreshed = false;
  try { await page.waitForFunction(({ key, count }) => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length > count, { key: game, count: beforeQueries }); refreshed = true; }
  finally { await screenshot(page, 'nonactive-game-external-global-refresh'); await record('nonactive-game-refresh-observation', { expected: { scopeStaysGame: true, followsGlobalShowPos: true, newQueryRequired: true }, actual: await page.evaluate(() => ({ scopeTrigger: document.querySelector('[data-ui-page="general-settings"] [role=combobox]')?.textContent, queryScopes: window.generalSettings.port.scopedPreferenceQueries, effectiveSnapshotScope: window.generalSettings.port.state.preferences.scopeKey, effectiveSnapshotShowPos: window.generalSettings.port.state.preferences.showPos, renderedShowPos: [...document.querySelectorAll('[data-ui-page="general-settings"] [role=switch]')].find(node => node.getAttribute('aria-label') === '显示词性标记')?.checked })), refreshed }); }
  assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.match(await scope(page).textContent(), /仅此游戏/); await setToggle(page, '显示日语注音', false); const dirtyReads = await page.evaluate(() => window.generalSettings.port.scopedPreferenceQueries.length);
  await page.evaluate(() => { const p = window.generalSettings.port, saved = { ...structuredClone(p.controlledScope('global').effective), scopeKey: 'global', showPos: false }; p.scopedPreferenceSaves.set('global', { saved, headRevision: 2, preferenceRevisionId: 'external-global-2' }); p.publish({ preferences: saved }); }); await settle(page); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await page.evaluate(() => window.generalSettings.port.scopedPreferenceQueries.length), dirtyReads); assert.equal(await scope(page).isDisabled(), true);
  await save(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示词性标记').isChecked(), false); assert.equal(await control(page, '显示日语注音').isChecked(), false); const saved = await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key).saved, game); assert.deepEqual(saved.overrides, { showRuby: true }); const facts = await stateFacts(page); await screenshot(page, 'game-dirty-after-external-global-update'); await page.close(); return { beforeQueries, refreshed, saved, facts };
});

check('GENERAL-15-external-global-updates-during-scope-query-coalesce-to-latest-authority', async () => {
  const page = await open(), game = 'game:general-structure-game'; await chooseScope(page, '仅此游戏 · サクラノ詩'); const initialCount = await page.evaluate(key => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length, game);
  await page.evaluate(key => { const p = window.generalSettings.port, saved = { ...structuredClone(p.controlledScope('global').effective), scopeKey: 'global', showPos: true }; p.scopedPreferenceSaves.set('global', { saved, headRevision: 1, preferenceRevisionId: 'external-queued-global-1' }); p.scopedPreferenceResponses.push({ scopeKey: key, holdKey: 'queued-global-read' }); p.publish({ preferences: saved }); }, game);
  await page.waitForFunction(() => window.generalSettings.port.scopedPreferencePending.has('queued-global-read')); assert.equal(await control(page, '显示日语注音').isDisabled(), true); assert.equal(await scope(page).isDisabled(), true); assert.equal(await control(page, '显示词性标记').isChecked(), false, 'A global projection alone cannot bypass the scoped query authority');
  await page.evaluate(() => { const p = window.generalSettings.port, saved = { ...structuredClone(p.controlledScope('global').effective), scopeKey: 'global', showPos: false, showRuby: false }; p.scopedPreferenceSaves.set('global', { saved, headRevision: 2, preferenceRevisionId: 'external-queued-global-2' }); p.publish({ preferences: saved }); }); await settle(page); assert.equal(await page.evaluate(key => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length, game), initialCount + 1, 'A pending scope read is not duplicated per projection render');
  await page.evaluate(() => window.generalSettings.port.releasePreferenceScope('queued-global-read')); await page.waitForFunction(({ key, count }) => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length > count + 1, { key: game, count: initialCount }); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await control(page, '显示词性标记').isChecked(), false); assert.equal(await scope(page).isEnabled(), true); assert.equal(await save(page).isDisabled(), true); const facts = await stateFacts(page); assert.deepEqual(facts.commands, []); assert.equal(facts.saved.find(([key]) => key === 'global')[1].headRevision, 2); await screenshot(page, 'queued-global-scope-query-latest'); await page.close(); return { initialCount, facts };
});

check('GENERAL-16-masked-global-change-refreshes-on-public-foreground-events-with-dirty-and-inflight-fences', async () => {
  const page = await open({ maskedScope: true }), game = 'game:general-structure-game', observation = { transport: 'private-memory-only', foregroundEvents: 'public window focus and document visibilitychange; no Native focus claim', stages: [] };
  const queries = () => page.evaluate(key => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length, game);
  const publicFocus = async () => { await page.bringToFront(); return page.evaluate(() => { window.focus(); window.dispatchEvent(new Event('focus')); document.dispatchEvent(new Event('visibilitychange')); return { visibility: document.visibilityState, hasFocus: document.hasFocus() }; }); };
  const externalGlobal = (headRevision, patch) => page.evaluate(({ headRevision, patch }) => {
    const p = window.generalSettings.port, before = JSON.stringify(p.state.preferences), saved = { ...structuredClone(p.controlledScope('global').effective), ...patch, scopeKey: 'global' };
    p.scopedPreferenceSaves.set('global', { saved, headRevision, preferenceRevisionId: 'masked-global-' + headRevision });
    p.publish({ preferences: p.controlledScope('game:general-active-game-b').effective, commitRevision: p.state.commitRevision + 1 });
    return { before, after: JSON.stringify(p.state.preferences), headRevision, patch };
  }, { headRevision, patch });
  try {
    await chooseScope(page, '仅此游戏 · サクラノ詩'); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor();
    assert.equal(await control(page, '显示日语注音').isChecked(), true); assert.equal(await control(page, '显示词性标记').isChecked(), false);
    const baseline = await queries(), update = await externalGlobal(2, { showRuby: false }); assert.equal(update.before, update.after, 'Active game B must mask the entire changed global projection'); await settle(page); assert.equal(await queries(), baseline, 'No subscription signal or foreground event means no speculative background polling');
    const focus = await publicFocus(); assert.deepEqual(focus, { visibility: 'visible', hasFocus: true }); observation.stages.push({ name: 'masked-clean-before-focus', baseline, update, focus });
    await page.waitForFunction(({ key, baseline }) => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length > baseline, { key: game, baseline }, { timeout: 3000 });
    await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), false);
    await setToggle(page, '显示词性标记', true); const dirtyCount = await queries(), dirtyUpdate = await externalGlobal(3, { showRuby: true }); assert.equal(dirtyUpdate.before, dirtyUpdate.after); await publicFocus(); await settle(page); assert.equal(await queries(), dirtyCount); assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await control(page, '显示日语注音').isChecked(), false);
    await discard(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示日语注音').isChecked(), true); assert.equal(await control(page, '显示词性标记').isChecked(), false); observation.stages.push({ name: 'dirty-focus-discard', dirtyCount, dirtyUpdate, afterQueries: await queries() });
    await setToggle(page, '显示日语注音', false); const saveCount = await queries(), saveUpdate = await externalGlobal(4, { showPos: true }); assert.equal(saveUpdate.before, saveUpdate.after); await publicFocus(); await settle(page); assert.equal(await queries(), saveCount); assert.equal(await control(page, '显示日语注音').isChecked(), false); assert.equal(await control(page, '显示词性标记').isChecked(), false);
    await save(page).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示词性标记').isChecked(), true); const saved = await page.evaluate(key => window.generalSettings.port.scopedPreferenceSaves.get(key).saved, game); assert.deepEqual(saved.overrides, { showRuby: true }); observation.stages.push({ name: 'dirty-focus-save', saved, saveCount, saveUpdate, afterQueries: await queries() });
    const heldCount = await queries(); await externalGlobal(5, { showPos: false }); await page.evaluate(key => window.generalSettings.port.scopedPreferenceResponses.push({ scopeKey: key, holdKey: 'foreground-held' }), game); await publicFocus(); await page.waitForFunction(() => window.generalSettings.port.scopedPreferencePending.has('foreground-held')); assert.equal(await scope(page).isDisabled(), true); assert.equal(await control(page, '显示词性标记').isChecked(), true);
    const heldUpdate = await externalGlobal(6, { showPos: true }); assert.equal(heldUpdate.before, heldUpdate.after); await page.evaluate(() => { for (let index = 0; index < 8; index++) { window.dispatchEvent(new Event('focus')); document.dispatchEvent(new Event('visibilitychange')); } }); await settle(page); assert.equal(await queries(), heldCount + 1, 'Foreground bursts coalesce while the first query is pending');
    await page.evaluate(() => window.generalSettings.port.releasePreferenceScope('foreground-held')); await page.waitForFunction(({ key, count }) => window.generalSettings.port.scopedPreferenceQueries.filter(value => value === key).length >= count + 2, { key: game, count: heldCount }); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); await settle(page); assert.equal(await queries(), heldCount + 2, 'An older read cannot acknowledge newer same-signature invalidations, and the burst is not a query storm'); assert.equal(await control(page, '显示词性标记').isChecked(), true); assert.equal(await control(page, '显示日语注音').isChecked(), false); observation.stages.push({ name: 'same-signature-inflight-coalescing', heldCount, heldUpdate, afterQueries: await queries() });
    await externalGlobal(7, { showPos: false }); await page.evaluate(key => window.generalSettings.port.scopedPreferenceResponses.push({ scopeKey: key, error: '受控返焦读取失败' }), game); await publicFocus(); await general(page).getByRole('button', { name: '重新读取偏好来源', exact: true }).waitFor(); const errorCount = await queries(); assert.equal(await control(page, '显示词性标记').isChecked(), true);
    const errorUpdate = await externalGlobal(8, { showRuby: false }); assert.equal(errorUpdate.before, errorUpdate.after); await publicFocus(); await settle(page); assert.equal(await queries(), errorCount, 'A failed foreground read remains explicitly recoverable; focus cannot erase the error or loop'); await general(page).getByRole('button', { name: '重新读取偏好来源', exact: true }).click(); await general(page).getByText('阅读偏好已保存', { exact: false }).waitFor(); assert.equal(await control(page, '显示词性标记').isChecked(), false); assert.equal(await control(page, '显示日语注音').isChecked(), false); observation.stages.push({ name: 'failed-foreground-read-and-explicit-recovery', errorCount, errorUpdate, afterQueries: await queries() });
    await page.getByRole('navigation', { name: '设置分类' }).getByRole('button', { name: '显示与快捷键', exact: true }).click(); await general(page).waitFor({ state: 'detached' }); const leavingCount = await queries(); await publicFocus(); await settle(page); assert.equal(await queries(), leavingCount, 'General foreground listeners must be detached outside General'); observation.stages.push({ name: 'section-listener-cleanup', leavingCount, afterQueries: await queries() });
    assert.deepEqual((await stateFacts(page)).commands.filter(command => !['save_scoped_preferences'].includes(command.kind)), []); return observation;
  } finally {
    observation.final = await page.evaluate(() => ({ queryScopes: window.generalSettings.port.scopedPreferenceQueries, preferences: window.generalSettings.port.state.preferences, selectedScope: document.querySelector('[data-ui-page="general-settings"] [role=combobox]')?.textContent, switches: [...document.querySelectorAll('[data-ui-page="general-settings"] [role=switch]')].map(node => ({ label: node.getAttribute('aria-label'), checked: node.checked, disabled: node.disabled })), visibility: document.visibilityState, hasFocus: document.hasFocus() }));
    await screenshot(page, 'masked-global-foreground-refresh'); await record('masked-global-foreground-observation', observation); await page.close();
  }
});

check('GENERAL-17-declared-inputs-reproduce-before-and-design-canvas-in-a-blank-workspace', async () => {
  const blank = path.join(output, 'blank-root'), requiredInputs = Object.keys(sourceBefore).sort(); assert.ok(requiredInputs.length > 100); assert.equal(requiredInputs.some(relative => path.isAbsolute(relative) || relative.startsWith('.build/') || relative.includes('..')), false);
  for (const relative of requiredInputs) { const destination = path.join(blank, relative); await fs.mkdir(path.dirname(destination), { recursive: true }); await fs.copyFile(path.join(root, relative), destination); }
  assert.deepEqual(await identities(blank), sourceBefore); await verifyPermanentInputs(blank);
  const directory = await reconstructCaptured(blank, path.join(blank, 'private-original-layout')), options = { width: 1000, height: 720, theme: 'dark', zoom: 1 }, left = await sourcePage(directory, options), right = await sourcePage(path.join(blank, promotedRelative), options);
  const a = await left.screenshot({ path: path.join(output, 'portable-original.png'), animations: 'disabled' }), b = await right.screenshot({ path: path.join(output, 'portable-promoted.png'), animations: 'disabled' });
  const chromiumDiagnostic = await exactPixels(a, b, 'portable-original-promoted-chromium-unqualified');
  const qualified = await captureGeneralSourceQualification({ base: blank, output: path.join(blank, '.build/qualified-source'), ...options });
  const diff = await exactPixels(qualified.left, qualified.right, 'portable-original-promoted'); assert.equal(diff.differingPixels, 0); await left.close(); await right.close();
  const beforeManifest = JSON.parse(await fs.readFile(path.join(blank, beforeRelative, 'manifest.json'), 'utf8')); for (const [name, entry] of Object.entries(beforeManifest.files)) assert.deepEqual(await fs.readFile(path.join(desktop, 'before-src', entry.original_path.split('/src/')[1])), await fs.readFile(path.join(blank, beforeRelative, name)));
  const proof = { declaredInputs: requiredInputs, inputHashes: sourceBefore, rootBuildInputs: [], undeclaredHistoricalArtifactInputs: [], blankRoot: path.relative(root, blank), canvas: diff, qualifiedObserver: qualified.receipt, chromiumDiagnostic: { qualification: 'not_qualified', ...chromiumDiagnostic }, privateBeforeLeaves: beforeManifest.files, native: 'not_run', database: 'not_run' }; await record('portability', proof); return proof;
});

check('GENERAL-18-real-main-consumer-opens-session-without-target-modal-and-retains-history-capture-revision-replay', async () => {
  const runs = [], failures = [];
  for (const mode of ['empty', 'stopped', 'active']) {
    const page = await open({ paired: true, width: 2000, height: 720, sessionMode: mode }), run = { mode, transport: 'shared typed private-memory port; both real WindowRoot consumers are mounted', stages: [] }, before = await stateFacts(page);
    const observe = async (stage, action) => { try { await action(); run.stages.push({ stage, status: 'passed', actual: await mainConsumerFacts(page) }); } catch (error) { const actual = await mainConsumerFacts(page); run.stages.push({ stage, status: 'failed', actual, error: error.stack }); failures.push({ mode, stage, actual, error: error.message }); await screenshot(page, `main-consumer-${mode}-${stage}-red`); } };
    try {
      await observe('history-compatible', async () => { await page.evaluate(() => window.generalSettings.port.openMainPage('history')); await expectMainPage(page, 'history'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); });
      await observe('capture-keeps-target-chooser', async () => { await page.evaluate(() => window.generalSettings.port.openMainPage('capture')); await expectMainPage(page, 'capture'); await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).waitFor(); });
      await observe('direct-session-clears-target-modal', async () => { await page.evaluate(() => window.generalSettings.port.openMainPage('session')); await expectMainPage(page, 'session'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); });
      // Reset through the retained public History intent, not by changing Main's internal page state.
      await page.evaluate(() => window.generalSettings.port.openMainPage('history')); await expectMainPage(page, 'history');
      await observe('general-link-reaches-real-session', async () => { await general(page).getByRole('button', { name: '当前会话', exact: true }).click(); await expectMainPage(page, 'session'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); assert.equal(await page.evaluate(() => window.generalSettings.port.mainPageRequests.at(-1).intent), 'session'); });
      await page.evaluate(() => window.generalSettings.port.openMainPage('history')); await expectMainPage(page, 'history');
      await observe('duplicate-and-old-events-do-not-reopen-modal', async () => { await page.evaluate(() => { const p = window.generalSettings.port, revision = p.mainPageRequests.at(-1).revision; p.emitMainRequest({ revision, intent: 'capture' }); p.emitMainRequest({ revision: revision - 1, intent: 'capture' }); p.emitMainRequest({ revision: revision - 1, intent: 'session' }); }); await settle(page); await expectMainPage(page, 'history'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); });
      await observe('new-capture-after-deduplication-and-history-dismissal', async () => { await page.evaluate(() => window.generalSettings.port.openMainPage('capture')); await expectMainPage(page, 'capture'); await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).waitFor(); await page.evaluate(() => window.generalSettings.port.openMainPage('history')); await expectMainPage(page, 'history'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); });
      await observe('cached-session-replay-after-remount', async () => { await page.evaluate(() => { const p = window.generalSettings.port, revision = p.mainPageRequests.at(-1).revision + 1; p.mainPageRequests.push({ revision, intent: 'session' }); window.generalSettings.remount(); }); await expectMainPage(page, 'session'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); });
      if (mode === 'active') await observe('late-subscription-receipt-and-old-consumer-callback', async () => {
        await page.evaluate(() => { window.generalSettings.port.consumerRegistrationHolds.push('old-main-registration'); window.generalSettings.remount(); }); await page.waitForFunction(() => window.generalSettings.port.consumerRegistrationPending.has('old-main-registration')); const oldIndex = await page.evaluate(() => window.generalSettings.port.consumerArchive.length - 1); await page.evaluate(() => window.generalSettings.remount()); await expectMainPage(page, 'session');
        await page.evaluate(index => { const p = window.generalSettings.port; p.emitArchivedMainRequest(index, { revision: p.mainPageRequests.at(-1).revision + 100, intent: 'capture' }); p.releaseConsumerRegistration('old-main-registration'); }, oldIndex); await page.waitForFunction(() => window.generalSettings.port.consumerListeners.size === 1); await settle(page); await expectMainPage(page, 'session'); assert.equal(await page.getByRole('dialog', { name: '选择正在运行的游戏', exact: true }).count(), 0); run.registration = await page.evaluate(() => ({ liveListeners: window.generalSettings.port.consumerListeners.size, cleanupCount: window.generalSettings.port.consumerCleanupCount, archived: window.generalSettings.port.consumerArchive.length }));
      });
      const after = await stateFacts(page); assert.deepEqual(after.session, before.session); assert.deepEqual(after.saved, before.saved); assert.deepEqual(after.commands, []); assert.deepEqual(after.startupWrites, []); run.noWrite = { sessionUnchanged: true, preferenceWrites: [], captureWrites: [], startupWrites: [] }; run.requests = after.nav; await screenshot(page, `main-consumer-${mode}-final`);
    } finally { runs.push(run); await record(`main-consumer-${mode}`, run); await page.close(); }
  }
  await record('main-consumer-summary', { runs, failures, native: 'not_run', database: 'not_run' }); assert.deepEqual(runs.map(run=>[run.mode,run.stages.length]), [['empty',7],['stopped',7],['active',8]], 'All three concrete session states and22 registered consumer stages must execute'); assert.deepEqual(failures, [], 'Actual MainWindow must consume Session separately from Capture in every session state'); return { runs, failures, native: 'not_run', database: 'not_run' };
});
