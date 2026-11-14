/** Actual SettingsWindow + Rust form policy; explicit controlled Native OAuth port.
 * This runner never claims a real Google account, browser callback, SQL or Vault acceptance. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { launchLockedBrowser, verifyBrowserIdentity } from './locked-browser.mjs';
import { startServiceFormRuntime } from '../support/service-form-runtime.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const desktop = path.join(root, 'apps/desktop');
const output = path.resolve(process.argv[2] ?? path.join(root, '.build/current-client-execution-20261105/cp06/google-interactive-flow/ui-' + Date.now()));
await fs.mkdir(output, { recursive: false });
const sources = ['apps/desktop/src/features/model-config/useGoogleAuthorization.ts', 'apps/desktop/src/features/model-config/ModelConfigScreen.tsx', 'apps/desktop/src/features/model-config/GuidedServiceEditor.tsx', 'apps/desktop/src/windows/settings/SettingsWindow.tsx', 'apps/desktop/src/bridge/application-port.ts', 'apps/desktop/src/bridge/tauri-port.ts', 'apps/desktop/src/bridge/contracts.generated.ts', 'apps/desktop/tests/support/design-port.ts', 'apps/desktop/tests/support/service-form-runtime.mjs', 'apps/desktop/tests/visual/verify-google-authorization-ui.mjs', 'crates/app-infra/src/providers/profiles.rs', 'crates/app-core/src/contracts/google_authorization_ui.rs', 'crates/app-core/src/domain/model_config/service_draft.rs', 'crates/app-core/src/domain/model_config/service_editor.rs'];
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const identities = async () => Object.fromEntries(await Promise.all(sources.map(async name => [name, sha(await fs.readFile(path.join(root, name)))])));
const before = await identities(), checks = [], screenshots = [], browserErrors = [];
let runtime, server, browser, browserIdentity, serverLog = '', origin;
async function command(args, label) {
  const child = spawn(args[0], args.slice(1), { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = ''; child.stdout.on('data', value => stdout += value); child.stderr.on('data', value => stderr += value);
  const exitCode = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); });
  await fs.writeFile(path.join(output, label + '.json'), JSON.stringify({ command: args, exitCode, stdout, stderr }, null, 2) + '\n');
  assert.equal(exitCode, 0, label + ': ' + stderr);
}
async function check(id, run) { try { const facts = await run(); checks.push({ id, status: 'passed', facts }); console.log(id + ': passed'); } catch (error) { checks.push({ id, status: 'failed', error: error.message, stack: error.stack }); console.log(id + ': failed: ' + error.stack); } }
async function open(mode = 'custom') {
  const page = await browser.newPage({ viewport: { width: 1000, height: 720 }, deviceScaleFactor: 1 });
  page.on('pageerror', error => browserErrors.push({ name: error.name, message: error.message }));
  await page.goto(origin + '?mode=' + mode);
  await page.locator('[data-service-field="oauth_login"]').waitFor();
  await page.waitForFunction(() => window.oauthUI.snapshot().calls.inspect.length > 0);
  await page.waitForFunction(() => !document.querySelector('[data-service-field="oauth_login"]')?.textContent.includes('正在核对 Google 授权状态'));
  return page;
}
async function capture(page, name) { await page.evaluate(() => document.fonts.ready); const file = name + '.png'; await page.screenshot({ path: path.join(output, file), animations: 'disabled' }); screenshots.push({ file, sha256: sha(await fs.readFile(path.join(output, file))), approved: false, native: false }); }
const login = page => page.locator('[data-service-field="oauth_login"]').getByRole('button', { name: '登录并授权Google', exact: true });
async function start(page) { await login(page).click(); await page.waitForFunction(() => window.oauthUI.snapshot().calls.start.length === 1); }
async function editClient(page, value) {
  const details = page.locator('[data-guided-service-editor] details').filter({ has: page.locator('summary').getByText('高级设置', { exact: true }) });
  if (await details.getAttribute('open') === null) await details.locator('summary').click();
  await page.locator('[data-service-field="oauth_client_id"] input').fill(value);
  await page.waitForFunction(() => document.querySelector('[data-guided-service-editor]')?.getAttribute('aria-busy') === 'false');
}
try {
  await command([process.execPath, path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit'], 'typecheck');
  await command([process.execPath, path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck-tests');
  runtime = await startServiceFormRuntime({ root, output });
  const catalogResponse = await runtime.query({ kind: 'catalog' }); assert.equal(catalogResponse.ok, true);
  const harness = `import React from ${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))};
import {createRoot} from ${JSON.stringify(path.join(desktop, 'node_modules/react-dom/client.js'))};
import {SettingsWindow} from ${JSON.stringify(path.join(desktop, 'src/windows/settings/SettingsWindow.tsx'))};
import {ApplicationStore} from ${JSON.stringify(path.join(desktop, 'src/bridge/projections/application-store.ts'))};
import {DesignPort} from ${JSON.stringify(path.join(desktop, 'tests/support/design-port.ts'))};
import ${JSON.stringify(path.join(desktop, 'src/ui/reset.css'))};
document.documentElement.dataset.windowRole='settings';
const mode=new URLSearchParams(location.search).get('mode'),port=new DesignPort('model-config'),catalog=${JSON.stringify(catalogResponse.value)};
port.catalogFixture=catalog;
const source={kind:'http',source:{revisionId:'controlled-saved-1',modelId:'configured-a'}},target={kind:'http',preset:catalog.presets.find(p=>p.key.presetId==='builtin.google-v3').key};
const client='controlled-public-a.apps.googleusercontent.com',grant={grantId:'controlled-owned-grant',authorizationEpoch:1};
let authentication=mode==='authorized'?{kind:'google_owned_grant',authorization:grant}:{kind:'none'},localRevoked=false;
const values={name:'Google 配置',project_id:'controlled-project',google_auth_method:'oauth',oauth_client_mode:mode==='builtin'?'application':'custom',oauth_client_id:client,proxy_mode:mode==='proxy'?'override':'inherit',...(mode==='proxy'?{proxy_url:'http://127.0.0.1:7890'}:{})};
const config=port.state.configuration.draft.configuration;config.revisionId=source.source.revisionId;config.models[0].modelId=source.source.modelId;config.connections[0].proxyMode=mode==='proxy'?'override':'inherit';config.connections[0].networkMetadata=mode==='proxy'?{proxyUrl:'http://127.0.0.1:7890'}:{};
const calls={inspect:[],start:[],status:[],cancel:[],revoke:[],save:[],select:[],enable:[],test:[],testCancel:[]},operations=new Map();
let heldStatus=null,releaseStatus=null,startMismatch=false,holdCancellation=false;
function op(id,state='authorizing',extra={}){return{operationId:id,state,message:'受控授权状态：'+state,canCancel:state==='authorizing',...extra}}
port.serviceFormFixture=async request=>{const response=await fetch('/form',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(request)});const result=await response.json();if(!result.ok)throw Error(result.code);return result.value};
port.serviceFixtures.configuration=async()=>({...port.state.configuration,profileGeneration:port.state.profileGeneration,draftHeadRevision:1,appliedHeadRevision:0});
port.serviceFixtures.saved=async query=>({profileGeneration:port.state.profileGeneration,appliedHeadRevision:0,selection:{requestedScope:query.scope,effectiveScope:{kind:'global'},selectionRevision:1,effectiveSelectionRevision:1,main:null,comparisons:[],purposeEnabled:false},rows:query.purpose==='translation'?[{source,logicalResourceId:'configured-a',displayName:'Google 配置',backend:'http_translation',purpose:'translation',eligibility:{kind:'blocked',reasons:[{code:'not_tested',message:'受控服务未测试',fieldId:null}]},verification:{kind:'untested'}}]:[]});
port.serviceFixtures.draft=async query=>({profileGeneration:port.state.profileGeneration,draftHeadRevision:1,source:query.source,editing:query.source,target,purpose:query.purpose,values,authentication,readonlyReason:null,affectedSharedModelIds:[],fallback:[]});
port.serviceFixtures.save=async input=>{calls.save.push(structuredClone(input));authentication=structuredClone(input.authentication);return{operationId:input.operationId,revisionId:source.source.revisionId,draftHeadRevision:1,savedResource:source,affectedSharedModelIds:[]}};
port.serviceFixtures.select=async input=>{calls.select.push(input);throw Error('No controlled selection is allowed by this scenario')};
port.serviceFixtures.enabled=async input=>{calls.enable.push(input);throw Error('No controlled purpose mutation is allowed')};
port.serviceFixtures.test=async(input,id)=>{calls.test.push({input,id});return new Promise(resolve=>window.finishControlledTest=(status='passed')=>resolve({source:input.source,purpose:input.purpose,testId:'controlled-test',status,stage:'controlled',message:status==='failed'?'Google 服务测试失败':status==='cancelled'?'测试已取消':'旧测试迟到结果，不应覆盖授权更新',modelIds:[],...(status==='failed'?{failure:{code:'google_authorization_not_ready',stage:'google_token',recovery:'请核对授权与网络，再主动重新测试。'},rawProviderResponse:'RAW_SECRET_MARKER_DO_NOT_RENDER'}:{})}))};
port.serviceFixtures.cancelTest=async(operationId,profileGeneration)=>{calls.testCancel.push({operationId,profileGeneration})};
if(mode!=='uninstalled'){
port.googleAuthorizationFixtures.inspect=async context=>{calls.inspect.push(structuredClone(context));const owned=context.authorization&&!localRevoked;return{state:owned?'authorized':context.clientMode==='application'?'unconfigured':'ready',canLogin:context.clientMode==='custom',canLogout:!!owned,message:owned?'已核对受控授权':context.clientMode==='application'?'应用未提供真实客户端资料，无法登录':'本地格式已准备；尚未进行真实 Google 授权',affectedServiceCount:owned?2:0,...(owned?{authorization:context.authorization}:{})}};
port.googleAuthorizationFixtures.start=async input=>{calls.start.push(structuredClone(input));const value=op(input.operationId);operations.set(input.operationId,value);return startMismatch?{...value,operationId:'foreign-operation'}:value};
port.googleAuthorizationFixtures.status=async query=>{calls.status.push(structuredClone(query));const value=structuredClone(operations.get(query.operationId));if(heldStatus===query.operationId)await new Promise(resolve=>releaseStatus=resolve);return value};
port.googleAuthorizationFixtures.cancel=async query=>{calls.cancel.push(structuredClone(query));const value=op(query.operationId,holdCancellation?'authorizing':'cancelled',{canCancel:false});operations.set(query.operationId,value);return value};
port.googleAuthorizationFixtures.revoke=async input=>{calls.revoke.push(structuredClone(input));localRevoked=true;return{localRevoked:true,remoteRevocationConfirmed:false,message:'本地已撤销；Google 远端撤销尚未确认。'}};
}
window.oauthUI={port,source,snapshot:()=>({calls,authentication,operations:[...operations],profileGeneration:port.state.profileGeneration}),complete:()=>{const id=calls.start.at(-1).operationId;operations.set(id,op(id,'authorized',{authorization:grant,canCancel:false}));},holdStatus:()=>{heldStatus=calls.start.at(-1).operationId;},releaseStatus:()=>{heldStatus=null;releaseStatus?.()},holdCancellation:()=>holdCancellation=true,finishCancellation:()=>{const id=calls.start.at(-1).operationId;operations.set(id,op(id,'cancelled',{canCancel:false}))},mismatch:()=>startMismatch=true,publish:patch=>port.publish(patch)};
createRoot(document.getElementById('root')).render(<SettingsWindow store={new ApplicationStore(port)}/>);`;
  await fs.writeFile(path.join(output, 'harness.tsx'), harness);
  await fs.writeFile(path.join(output, 'index.html'), '<!doctype html><html lang="zh-CN"><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><style>*{box-sizing:border-box}html,body,#root{margin:0;width:100%;height:100%;overflow:hidden;font-family:-apple-system,BlinkMacSystemFont,sans-serif;color:var(--material-text);background:var(--material-shell-fill)}</style><div id="root"></div><script type="module" src="/harness.tsx"></script></html>');
  const configFile = path.join(output, 'vite.config.mjs');
  await fs.writeFile(configFile, `import react from ${JSON.stringify(path.join(desktop, 'node_modules/@vitejs/plugin-react/dist/index.js'))};export default{root:${JSON.stringify(output)},cacheDir:${JSON.stringify(path.join(output, 'vite-cache'))},plugins:[react()],resolve:{alias:[{find:/^react$/,replacement:${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))}},{find:/^react\\/jsx-runtime$/,replacement:${JSON.stringify(path.join(desktop, 'node_modules/react/jsx-runtime.js'))}}]},build:{outDir:${JSON.stringify(path.join(output, 'bundle'))},emptyOutDir:true,target:'es2022'},preview:{host:'127.0.0.1',proxy:{'/form':${JSON.stringify(runtime.origin)}}}};`);
  await command([process.execPath, path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', configFile], 'controlled-build');
  const free = createServer(); await new Promise(resolve => free.listen(0, '127.0.0.1', resolve)); const port = free.address().port; await new Promise(resolve => free.close(resolve)); origin = 'http://127.0.0.1:' + port;
  server = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--config', configFile, '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', value => serverLog += value); server.stderr.on('data', value => serverLog += value);
  for (let attempt = 0; attempt < 120; attempt++) { try { if ((await fetch(origin)).ok) break; } catch {} if (attempt === 119 || server.exitCode !== null) throw Error(serverLog); await new Promise(resolve => setTimeout(resolve, 100)); }
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());
  await check('GOOGLE-UI-01-built-in-is-blocked-without-client', async () => {
    const page = await open('builtin'); assert.equal(await login(page).isDisabled(), true); await page.getByText('应用未提供真实客户端资料，无法登录', { exact: true }).waitFor();
    assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.start)).length, 0); await capture(page, 'builtin-blocked'); await page.close(); return { browserOpens: 0, actualGoogleAccount: false };
  });
  await check('GOOGLE-UI-02-explicit-login-only-updates-editor-until-save', async () => {
    const page = await open(); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.start)).length, 0); await start(page); await capture(page, 'explicit-login-pending');
    await page.evaluate(() => window.oauthUI.complete()); await page.getByText('Google 授权已完成，当前编辑已更新。请保存配置，再从统一选择器明确选用。', { exact: true }).waitFor();
    let facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.save.length, 0); assert.equal(facts.calls.select.length, 0); assert.equal(facts.calls.enable.length, 0); assert.equal(facts.calls.cancel.length, 0);
    await page.locator('[data-guided-service-page]>footer').getByRole('button', { name: '保存配置', exact: true }).click(); await page.waitForFunction(() => window.oauthUI.snapshot().calls.save.length === 1);
    facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.deepEqual(facts.calls.save[0].authentication, { kind: 'google_owned_grant', authorization: { grantId: 'controlled-owned-grant', authorizationEpoch: 1 } }); assert.equal(facts.calls.select.length, 0); assert.equal(facts.calls.enable.length, 0);
    await capture(page, 'authorized-not-selected'); await page.close(); return { explicitClick: true, editorOnlyUntilExplicitSave: true, selectorAndPurposeUnchanged: true };
  });
  await check('GOOGLE-UI-03-client-change-cancels-and-rejects-late-authorized-result', async () => {
    const page = await open(); await start(page); await page.evaluate(() => { window.oauthUI.complete(); window.oauthUI.holdStatus(); }); await page.waitForFunction(() => window.oauthUI.snapshot().calls.status.length > 0);
    await editClient(page, 'controlled-public-b.apps.googleusercontent.com'); await page.waitForFunction(() => window.oauthUI.snapshot().calls.cancel.length > 0); await page.evaluate(() => window.oauthUI.releaseStatus()); await page.waitForTimeout(600);
    assert.equal(await page.getByText('Google 授权已完成，当前编辑已更新。请保存配置，再从统一选择器明确选用。', { exact: true }).count(), 0);
    await page.locator('[data-guided-service-page]>footer').getByRole('button', { name: '保存配置', exact: true }).click(); await page.getByRole('alert').filter({ hasText: '尚未完成此服务的授权' }).waitFor(); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.save)).length, 0);
    await capture(page, 'late-authorized-ignored'); await page.close(); return { oldOperationCancelled: true, lateReferenceNotSaved: true };
  });
  await check('GOOGLE-UI-04-cancellation-waits-for-real-terminal-projection', async () => {
    const page = await open(); await page.evaluate(() => window.oauthUI.holdCancellation()); await start(page); await page.getByRole('button', { name: '取消授权', exact: true }).click();
    await page.getByRole('button', { name: '正在安全收尾…', exact: true }).waitFor(); assert.equal(await login(page).isDisabled(), true); assert.equal(await page.locator('[data-google-authorization-state]').getAttribute('data-google-authorization-state'), 'authorizing');
    await page.evaluate(() => window.oauthUI.finishCancellation()); await page.locator('[data-google-authorization-state="cancelled"]').waitFor(); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.save)).length, 0); await capture(page, 'cancelled-after-drain'); await page.close(); return { noInventedCancelledWhileDraining: true };
  });
  await check('GOOGLE-UI-05-leaving-page-cancels-without-reopening-browser', async () => {
    const page = await open(); await start(page); await page.getByRole('button', { name: '通用', exact: true }).click(); await page.waitForFunction(() => window.oauthUI.snapshot().calls.cancel.length > 0);
    await page.evaluate(() => window.oauthUI.complete()); await page.getByRole('button', { name: '翻译服务', exact: true }).click(); await page.locator('[data-service-field="oauth_login"]').waitFor(); await page.waitForTimeout(700);
    const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.start.length, 1); assert.equal(facts.calls.save.length, 0); assert.equal(await page.getByText('Google 授权已完成，当前编辑已更新。请保存配置，再从统一选择器明确选用。', { exact: true }).count(), 0); await page.close(); return { explicitOldOperationCancelled: true, noAutomaticRestart: true };
  });
  await check('GOOGLE-UI-06-profile-change-isolates-late-operation', async () => {
    const page = await open(); await start(page); await page.evaluate(() => window.oauthUI.publish({ profileGeneration: 2, profileId: 'controlled-next-profile' })); await page.waitForFunction(() => window.oauthUI.snapshot().calls.cancel.length > 0);
    await page.evaluate(() => window.oauthUI.complete()); await page.waitForTimeout(700); const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.cancel[0].expectedProfileGeneration, 1); assert.equal(facts.calls.start.length, 1); assert.equal(facts.calls.save.length, 0); assert.ok(facts.calls.inspect.some(value => value.expectedProfileGeneration === 2)); await page.close(); return { originalGenerationCancellation: true, newProfileNoInheritedAuthorization: true };
  });
  await check('GOOGLE-UI-07-shared-revoke-requires-confirmation-and-shows-partial-remote-result', async () => {
    const page = await open('authorized'); await page.getByRole('button', { name: '解除授权…', exact: true }).click(); await page.getByRole('heading', { name: '解除 Google 授权？', exact: true }).waitFor(); await page.getByText('当前配置档有 2 项已保存服务引用这项授权。', { exact: true }).waitFor(); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.revoke)).length, 0); await capture(page, 'shared-revocation-review');
    await page.getByRole('dialog').getByRole('button', { name: '取消', exact: true }).click(); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.revoke)).length, 0);
    await page.getByRole('button', { name: '解除授权…', exact: true }).click(); await page.getByRole('button', { name: '确认解除授权', exact: true }).click(); await page.getByText('本地已撤销；Google 远端撤销尚未确认。', { exact: true }).waitFor(); const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.revoke.length, 1); assert.equal(facts.calls.save.length, 0); assert.equal(facts.calls.select.length, 0); assert.equal(facts.calls.enable.length, 0); await page.close(); return { trueImpactProjection: 2, confirmationRequired: true, remoteFailureNotMisreported: true };
  });
  await check('GOOGLE-UI-08-current-connection-proxy-is-forwarded', async () => {
    const page = await open('proxy'); await start(page); const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.deepEqual(facts.calls.inspect[0].proxy, { mode: 'override', proxyUrl: 'http://127.0.0.1:7890' }); assert.deepEqual(facts.calls.start[0].context.proxy, facts.calls.inspect[0].proxy); await page.close(); return { exactExistingConnectionPolicy: true, googleSpecificProxyFormCreated: false };
  });
  await check('GOOGLE-UI-09-authentication-method-change-cancels', async () => {
    const page = await open(); await start(page); await page.locator('[data-service-field="google_auth_method"]').getByRole('combobox').click(); await page.getByRole('option', { name: '使用服务账号', exact: true }).click(); await page.waitForFunction(() => window.oauthUI.snapshot().calls.cancel.length > 0); await page.locator('[data-service-field="service_account_json"]').waitFor(); await page.evaluate(() => window.oauthUI.complete()); await page.waitForTimeout(600); assert.equal(await page.locator('[data-service-field="oauth_login"]').count(), 0); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.save)).length, 0); await page.close(); return { methodBoundary: true, lateOAuthNotAppliedToServiceAccount: true };
  });
  await check('GOOGLE-UI-10-foreign-operation-response-fails-closed', async () => {
    const page = await open(); await page.evaluate(() => window.oauthUI.mismatch()); await start(page); await page.locator('[data-google-authorization-state="cancelled"]').waitFor(); const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.cancel[0].operationId, facts.calls.start[0].operationId); assert.equal(facts.calls.save.length, 0); assert.equal(facts.calls.select.length, 0); await page.close(); return { operationIdentityChecked: true };
  });
  await check('GOOGLE-UI-11-authorization-update-invalidates-old-service-test', async () => {
    const page = await open(); await page.getByRole('button', { name: '翻译一句试试', exact: true }).click(); await page.waitForFunction(() => window.oauthUI.snapshot().calls.test.length === 1); await start(page); await page.evaluate(() => window.oauthUI.complete()); await page.getByText('Google 授权已完成，当前编辑已更新。请保存配置，再从统一选择器明确选用。', { exact: true }).waitFor();
    await page.waitForFunction(() => window.oauthUI.snapshot().calls.testCancel.length === 1); await page.evaluate(() => window.finishControlledTest()); await page.waitForTimeout(100); const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.testCancel[0].operationId, facts.calls.test[0].id); assert.equal(facts.calls.testCancel[0].profileGeneration, 1); assert.equal(await page.getByText('旧测试迟到结果，不应覆盖授权更新', { exact: true }).count(), 0); assert.equal(await page.getByRole('button', { name: '翻译一句试试', exact: true }).isDisabled(), true); assert.equal(facts.calls.save.length, 0); await page.close(); return { testEpochAdvanced: true, exactOldTestCancelled: true, dirtyAuthorizationNotOverwritten: true };
  });
  await check('GOOGLE-UI-12-client-change-clears-editor-reference-without-revoking-shared-grant', async () => {
    const page = await open('authorized'); await editClient(page, 'controlled-new-owner.apps.googleusercontent.com'); await page.waitForFunction(() => window.oauthUI.snapshot().calls.inspect.some(value => value.publicClientId === 'controlled-new-owner.apps.googleusercontent.com'));
    const facts = await page.evaluate(() => window.oauthUI.snapshot()); assert.equal(facts.calls.inspect.at(-1).authorization, undefined); assert.equal(facts.calls.revoke.length, 0); assert.equal(facts.calls.start.length, 0); await page.locator('[data-guided-service-page]>footer').getByRole('button', { name: '保存配置', exact: true }).click(); await page.getByRole('alert').filter({ hasText: '尚未完成此服务的授权' }).waitFor(); assert.equal((await page.evaluate(() => window.oauthUI.snapshot().calls.save)).length, 0); await page.close(); return { editorReferenceCleared: true, sharedAuthorizationUntouched: true, noImplicitLogin: true };
  });
  await check('GOOGLE-UI-13-safe-service-test-failure-detail-is-visible-without-raw-response', async () => {
    const page=await open('authorized');const client=await page.locator('[data-service-field="oauth_client_id"] input').inputValue();
    await page.getByRole('button',{name:'翻译一句试试',exact:true}).click();await page.waitForFunction(()=>window.oauthUI.snapshot().calls.test.length===1);await page.evaluate(()=>window.finishControlledTest('failed'));
    await page.getByRole('alert').filter({hasText:'请核对授权与网络，再主动重新测试。'}).waitFor();await page.getByText('google_authorization_not_ready · google_token',{exact:true}).waitFor();
    assert.equal((await page.locator('body').textContent()).includes('RAW_SECRET_MARKER_DO_NOT_RENDER'),false);assert.equal(await page.locator('[data-service-field="oauth_client_id"] input').inputValue(),client);const facts=await page.evaluate(()=>window.oauthUI.snapshot());assert.equal(facts.calls.save.length,0);assert.equal(facts.calls.select.length,0);assert.equal(facts.calls.enable.length,0);await capture(page,'safe-service-test-failure');await page.close();return{codeStageRecoveryVisible:true,rawProviderResponseHidden:true,inputAndSelectionPreserved:true};
  });
  await check('GOOGLE-UI-14-cancelled-service-test-remains-neutral-without-failure-detail', async () => {
    const page=await open('authorized');await page.getByRole('button',{name:'翻译一句试试',exact:true}).click();await page.waitForFunction(()=>window.oauthUI.snapshot().calls.test.length===1);await page.evaluate(()=>window.finishControlledTest('cancelled'));await page.getByText('测试已取消',{exact:true}).waitFor();assert.equal(await page.getByRole('alert').count(),0);assert.equal((await page.locator('body').textContent()).includes('google_authorization_not_ready'),false);await page.close();return{cancelledNeutral:true,noFabricatedFailure:true};
  });
  await check('GOOGLE-UI-15-proxy-edit-cancels-old-oauth-context', async () => {
    const page=await open('proxy');await start(page);const advanced=page.locator('[data-guided-service-editor] details').filter({has:page.locator('summary').getByText('高级设置',{exact:true})});if(await advanced.getAttribute('open')===null)await advanced.locator('summary').click();await page.locator('[data-service-field="proxy_url"] input').fill('http://127.0.0.1:7900');await page.waitForFunction(()=>window.oauthUI.snapshot().calls.cancel.length>0);await page.waitForFunction(()=>window.oauthUI.snapshot().calls.inspect.some(value=>value.proxy?.proxyUrl==='http://127.0.0.1:7900'));const facts=await page.evaluate(()=>window.oauthUI.snapshot());assert.equal(facts.calls.cancel[0].operationId,facts.calls.start[0].operationId);assert.equal(facts.calls.start.length,1);assert.equal(facts.calls.save.length,0);await page.close();return{proxyContextIsolated:true,explicitNewLoginRequired:true};
  });
  await check('INDEPENDENT-GOOGLE-UI-16-cleanup-pending-blocks-login-without-adopting-reference',async()=>{
    const page=await open();await page.evaluate(()=>{const prior=window.oauthUI.port.googleAuthorizationFixtures.status;window.oauthUI.port.googleAuthorizationFixtures.status=async query=>({...await prior(query),state:'cleanup_pending',message:'本次新授权尚待安全清理',authorization:undefined,canCancel:false});});await start(page);await page.locator('[data-google-authorization-state="cleanup_pending"]').waitFor();assert.equal(await login(page).isDisabled(),true);assert.equal(await page.getByRole('button',{name:'取消授权',exact:true}).count(),0);await page.getByText('本次新授权尚待安全清理',{exact:true}).waitFor();const facts=await page.evaluate(()=>window.oauthUI.snapshot());assert.equal(facts.calls.start.length,1);assert.equal(facts.calls.save.length,0);assert.equal(facts.calls.select.length,0);await capture(page,'independent-cleanup-pending');await page.close();return{actualSettingsState:'cleanup_pending',retryBlocked:true,noGrantAdoption:true,nativeControlled:true};
  });
  await check('INDEPENDENT-GOOGLE-UI-17-leaving-during-held-start-reply-cancels-and-rejects-late-result',async()=>{
    const page=await open();await page.evaluate(()=>{const prior=window.oauthUI.port.googleAuthorizationFixtures.start;window.oauthUI.port.googleAuthorizationFixtures.start=async input=>{const value=await prior(input);return new Promise(resolve=>window.releaseIndependentStart=()=>resolve(value));};});await start(page);await page.getByRole('button',{name:'通用',exact:true}).click();await page.waitForFunction(()=>window.oauthUI.snapshot().calls.cancel.length>0);await page.evaluate(()=>window.releaseIndependentStart());await page.waitForTimeout(500);const facts=await page.evaluate(()=>window.oauthUI.snapshot());assert.equal(facts.calls.start.length,1);assert.ok(facts.calls.cancel.every(row=>row.operationId===facts.calls.start[0].operationId));assert.equal(facts.calls.save.length,0);assert.equal(facts.calls.select.length,0);assert.equal(facts.calls.enable.length,0);await page.close();return{exactRequestedOperationCancelledBeforeReply:true,lateResultNotAdopted:true,nativeControlled:true};
  });
  assert.deepEqual(browserErrors, []); await verifyBrowserIdentity(browserIdentity);
} catch (error) { checks.push({ id: 'setup-or-completion', status: 'failed', error: error.message, stack: error.stack }); }
finally {
  if (browser) await browser.close();
  if (server) { server.kill('SIGTERM'); await new Promise(resolve => server.once('close', resolve)); }
  if (runtime) await runtime.close();
  const after = await identities(), sourceStable = JSON.stringify(before) === JSON.stringify(after);
  if (!sourceStable) checks.push({ id: 'source-stability', status: 'failed' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  const result = { scope: 'Actual SettingsWindow and Rust form authority over controlled Native authorization transport; not actual Google, SQL, Vault or system browser acceptance', status: checks.length && checks.every(value => value.status === 'passed') ? 'passed' : 'failed', before, after, sourceStable, checks, screenshots, browserIdentity, browserErrors, actualGoogleAccount: 'blocked_no_account_or_client_id', productAccepted: false };
  await fs.writeFile(path.join(output, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(output); if (result.status !== 'passed') process.exitCode = 1;
}
