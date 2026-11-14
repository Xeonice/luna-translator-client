/** CP07 component journeys over actual Rust projections; not full-page, Native or account acceptance. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { launchLockedBrowser, verifyBrowserIdentity } from './locked-browser.mjs';
import { startServiceFormRuntime } from '../support/service-form-runtime.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..'), desktop = path.join(root, 'apps/desktop');
const output = path.resolve(process.argv[2] ?? path.join(root, '.build/current-client-execution-20261105/guided-ui/run-' + Date.now()));
await fs.mkdir(output, { recursive: false });
const owned = ['apps/desktop/src/features/model-config/ServiceCatalogPicker.tsx','apps/desktop/src/features/model-config/GuidedServiceEditor.tsx','apps/desktop/src/features/model-config/guided.module.css','apps/desktop/tests/visual/verify-guided-services.mjs','apps/desktop/tests/support/service-form-runtime.mjs','crates/app-infra/examples/service_form_fixture.rs','apps/desktop/src/ui/primitives/Controls.tsx','apps/desktop/src/ui/primitives/controls.module.css'];
const policySources = ['apps/desktop/src/bridge/contracts.generated.ts','crates/app-core/src/contracts/service_form.rs','crates/app-core/src/domain/model_config/form.rs','crates/app-core/src/domain/model_config/catalog.rs','crates/app-core/src/contracts/service_catalog.rs','crates/app-infra/src/providers/registry.rs','crates/app-infra/src/providers/profiles.rs','crates/app-infra/src/providers/presets.rs','crates/app-infra/src/providers/register.rs'];
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const identities = async () => Object.fromEntries(await Promise.all([...owned, ...policySources].map(async file => [file, sha(await fs.readFile(path.join(root, file)))])));
const before = await identities(), checks = [], screenshots = [], browserErrors = [];
let runtime, server, browser, browserIdentity, serverLog = '';
async function command(executable, args, label, cwd = root) {
  const child = spawn(executable, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'] }); let stdout = '', stderr = '';
  child.stdout.on('data', data => stdout += data); child.stderr.on('data', data => stderr += data);
  const exitCode = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); });
  await fs.writeFile(path.join(output, `${label}.json`), JSON.stringify({ command: [executable, ...args], exitCode, stdout, stderr }, null, 2) + '\n');
  assert.equal(exitCode, 0, `${label} failed: ${stderr || stdout}`);
}
async function check(id, execute) { try { const facts = await execute(); checks.push({ id, status: 'passed', facts }); console.log(`${id}: passed`); } catch (error) { checks.push({ id, status: 'failed', error: error.message, stack: error.stack }); console.log(`${id}: failed: ${error.stack}`); } }
async function select(page, field, label) { await page.locator(`[data-service-field="${field}"]`).getByRole('combobox').click(); await page.getByRole('option', { name: label, exact: true }).click(); }
async function ready(page) { await page.waitForFunction(() => !!window.uiGuided && !window.uiGuided.snapshot().loading && (!!window.uiGuided.snapshot().projection || !!window.uiGuided.snapshot().error)); const state = await page.evaluate(() => window.uiGuided.snapshot()); assert.equal(state.error, null, 'Real Rust projection/transport failed'); assert.ok(state.projection); }
async function choose(page, target, purpose = 'translation') { await page.evaluate(({ target, purpose }) => window.uiGuided.choose({ target, purpose }), { target, purpose }); await ready(page); }
async function capture(page, name) { await page.evaluate(() => document.fonts.ready); const file = `${name}.png`; await page.screenshot({ path: path.join(output, file), animations: 'disabled' }); screenshots.push({ path: file, sha256: sha(await fs.readFile(path.join(output, file))), viewport: page.viewportSize(), approved: false, packagedNative: false }); }
async function open(viewport = { width: 1000, height: 720 }) { const page = await browser.newPage({ viewport, deviceScaleFactor: 1 }); page.on('pageerror', error => browserErrors.push({ name: error.name, message: error.message, stack: error.stack })); await page.goto(origin); await ready(page); return page; }
let origin;
try {
  await command(process.execPath, [path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit'], 'typecheck', desktop);
  runtime = await startServiceFormRuntime({ root, output });
  const catalogResponse = await runtime.query({ kind: 'catalog' }); assert.equal(catalogResponse.ok, true); const catalog = catalogResponse.value;
  await fs.writeFile(path.join(output, 'actual-rust-catalog.json'), JSON.stringify(catalog, null, 2) + '\n');
  const harness = `import React,{useCallback,useEffect,useRef,useState} from ${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))};
import {createRoot} from ${JSON.stringify(path.join(desktop, 'node_modules/react-dom/client.js'))};
import {ServiceCatalogPicker} from ${JSON.stringify(path.join(desktop, 'src/features/model-config/ServiceCatalogPicker.tsx'))};
import {GuidedServiceEditor,useGuidedServiceForm} from ${JSON.stringify(path.join(desktop, 'src/features/model-config/GuidedServiceEditor.tsx'))};
import {Button,Select,Sheet} from ${JSON.stringify(path.join(desktop, 'src/ui/primitives/Controls.tsx'))};
import ${JSON.stringify(path.join(desktop, 'src/ui/tokens/generated.css'))};
const catalog=${JSON.stringify(catalog)};
function SharedSheetProbe(){const[kind,setKind]=useState(null),[nested,setNested]=useState(false),[choice,setChoice]=useState('one');return <><nav aria-label="共享弹层回归入口">{['selection','confirmation','library','preview'].map(value=><Button key={value} onPress={()=>setKind(value)}>{'共享弹层 '+value}</Button>)}</nav><Sheet title={'共享弹层 '+kind} description="共享生产控件回归，测试数据不代表真实保存或授权。" open={kind!==null} onClose={()=>setKind(null)} appearance={kind==='library'?'library':kind==='preview'?'preview':'default'} previewAnchor={kind==='preview'?{right:620,top:80}:undefined} footer={<><Button onPress={()=>setKind(null)}>返回</Button><Button intent="primary" onPress={()=>setKind(null)}>完成</Button></>}>{kind==='selection'?<Select label="回归选项" value={choice} onChange={setChoice} options={[{key:'one',label:'第一项'},{key:'two',label:'第二项'}]}/>:null}<Button onPress={()=>setNested(true)}>打开内层确认</Button>{Array.from({length:40},(_,index)=><p key={index}>第{index+1}条完整说明。长内容仅由弹层正文区域滚动，顶部标题和底部操作始终可见。</p>)}<Button>末项操作</Button></Sheet><Sheet title="内层确认" open={nested} onClose={()=>setNested(false)} footer={<Button onPress={()=>setNested(false)}>取消内层</Button>}><p>仅验证嵌套语义与焦点恢复。</p></Sheet></>}
function App(){const [selection,setSelection]=useState({target:{kind:'http',preset:catalog.presets[0].key},purpose:'translation'}),[values,setValues]=useState({}),[picker,setPicker]=useState(false),[result,setResult]=useState('');const delay=useRef(0),reject=useRef(false),foreign=useRef(false),secretEvents=useRef([]),actions=useRef([]),queries=useRef([]),sequence=useRef(0),[resource,setResource]=useState(0);
const choose=useCallback(next=>{setSelection(next);setValues({});setResult('');setResource(n=>n+1);setPicker(false)},[]);
const project=useCallback(async request=>{const wait=delay.current;delay.current=0;const fail=reject.current;reject.current=false;const misroute=foreign.current;foreign.current=false;queries.current.push(request);const transportRequest=misroute?{target:{kind:'offline',engineId:catalog.offlineEngines[0].engineId,contractRevision:catalog.offlineEngines[0].contractRevision},purpose:'translation',values:{}}:request;const response=await fetch('/form',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(transportRequest)});const row=await response.json();if(wait)await new Promise(r=>setTimeout(r,wait));if(fail)throw Error('controlled rejected transport');if(!row.ok)throw Error(row.code);return row.value},[]);
const request=selection?{...selection,values}:null,form=useGuidedServiceForm(request,project);const fieldActions=Object.fromEntries((form.projection?.fields??[]).filter(row=>['action','path','secret'].includes(row.descriptor.fieldType)).map(row=>[row.descriptor.id,{enabled:true,onPress:()=>{actions.current.push(row.descriptor.id);setResult('已收到受控动作请求。')}}]));
useEffect(()=>{window.uiGuided={catalog,choose,snapshot:()=>({selection,values,loading:form.loading,error:form.error,projection:form.projection,queries:queries.current,secretEvents:secretEvents.current,actions:actions.current}),delayNext:ms=>delay.current=ms,rejectNext:()=>reject.current=true,foreignNext:()=>foreign.current=true,setValues,clear:()=>setSelection(null),showPicker:()=>setPicker(true)}},[choose,selection,values,form]);
return <><header className="heading"><h1>翻译服务</h1><p>选择你要使用的服务，再填写必要的信息。</p><button onClick={()=>setPicker(true)}>添加服务…</button></header><main>{selection?<GuidedServiceEditor resourceKey={String(resource)} title={selection.target.kind==='http'?catalog.presets.find(p=>p.key.presetId===selection.target.preset.presetId).label:catalog.offlineEngines.find(e=>e.engineId===selection.target.engineId).label} subtitle="受控组件回归 · 实际Rust字段投影" projection={form.projection} values={values} loading={form.loading} error={form.error} canSave={true} canTest={true} onChange={(id,value)=>setValues(old=>({...old,[id]:value}))} onSecretChange={(id,value)=>secretEvents.current.push({id,length:value.length})} fieldActions={fieldActions} onSave={()=>{actions.current.push('save');setResult('已收到保存请求；此回调未持久化或切换服务。')}} onTest={()=>{actions.current.push('test');setResult('已收到测试请求；此回调未调用真实翻译账号。')}} onCancel={()=>setSelection(null)} onChangeService={()=>setPicker(true)} result={result}/>:<p role="status">编辑已关闭。</p>}</main><ServiceCatalogPicker catalog={catalog} open={picker} initialPurpose={selection?.purpose??'translation'} onClose={()=>setPicker(false)} onSelect={choose}/><SharedSheetProbe/></>}
createRoot(document.getElementById('root')).render(<App/>);`;
  await fs.writeFile(path.join(output, 'harness.tsx'), harness);
  await fs.writeFile(path.join(output, 'index.html'), '<!doctype html><html lang="zh-CN"><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="icon" href="data:,"><title>Guided service production components</title><style>*{box-sizing:border-box}html,body,#root{margin:0;width:100%;height:100%;font-family:-apple-system,BlinkMacSystemFont,sans-serif;color:var(--material-text);background:var(--material-shell-fill)}#root{display:flex;flex-direction:column;overflow:hidden}.heading{padding:22px 28px 15px;border-bottom:1px solid var(--material-line);flex:none}.heading h1{font-size:23px;margin:0 0 6px}.heading p{font-size:12px;color:var(--material-text-secondary);margin:0 0 10px}main{padding:20px 28px;overflow:auto;min-height:0;flex:1}</style><div id="root"></div><script type="module" src="/harness.tsx"></script></html>');
  const config = path.join(output, 'vite.config.mjs');
  const bundle = path.join(output, 'bundle');
  await fs.writeFile(config, `import react from ${JSON.stringify(path.join(desktop, 'node_modules/@vitejs/plugin-react/dist/index.js'))}; export default {root:${JSON.stringify(output)},cacheDir:${JSON.stringify(path.join(output, 'vite-cache'))},plugins:[react()],resolve:{alias:[{find:/^react$/,replacement:${JSON.stringify(path.join(desktop, 'node_modules/react/index.js'))}},{find:/^react\\/jsx-runtime$/,replacement:${JSON.stringify(path.join(desktop, 'node_modules/react/jsx-runtime.js'))}}]},build:{outDir:${JSON.stringify(bundle)},emptyOutDir:true,target:'es2022'},server:{host:'127.0.0.1',fs:{allow:[${JSON.stringify(root)}]},proxy:{'/form':${JSON.stringify(runtime.origin)}}},preview:{proxy:{'/form':${JSON.stringify(runtime.origin)}}}};`);
  await command(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', config], 'controlled-build', desktop);
  const free = createServer(); await new Promise(resolve => free.listen(0, '127.0.0.1', resolve)); const port = free.address().port; await new Promise(resolve => free.close(resolve)); origin = `http://127.0.0.1:${port}`;
  server = spawn(process.execPath, [path.join(desktop, 'node_modules/vite/bin/vite.js'), 'preview', '--config', config, '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] }); server.stdout.on('data', data => serverLog += data); server.stderr.on('data', data => serverLog += data);
  for (let i = 0; i < 120; i++) { try { if ((await fetch(origin)).ok) break; } catch {} if (i === 119 || server.exitCode !== null) throw Error(`Controlled Vite failed: ${serverLog}`); await new Promise(resolve => setTimeout(resolve, 100)); }
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());
  const smoke = await open(); await smoke.close();
  await check('GUIDE-01-complete-real-rust-catalogue-and-rendering', async () => {
    assert.equal(catalog.protocols.length, 13); assert.equal(catalog.presets.length, 21); assert.equal(catalog.offlineEngines.length, 4);
    const page = await open(); let fields = 0, services = [];
    for (const entry of [...catalog.presets.map(p => ({ target: { kind: 'http', preset: p.key }, descriptor: p })), ...catalog.offlineEngines.map(e => ({ target: { kind: 'offline', engineId: e.engineId, contractRevision: e.contractRevision }, descriptor: e }))]) {
      await choose(page, entry.target); const projected = await page.evaluate(() => window.uiGuided.snapshot().projection); fields += projected.fields.length;
      assert.deepEqual(projected.target, entry.target); assert.equal(projected.purpose, 'translation');
      for (const field of projected.fields.filter(row => row.descriptor.classification === 'not_applicable' || row.descriptor.group === 'internal')) assert.equal(await page.locator(`[data-service-field="${field.descriptor.id}"]`).count(), 0);
      const textareaSecret = await page.locator('[data-guided-service-editor] input[type="password"]').count(); assert.equal(textareaSecret > 0, projected.fields.some(row => row.visibility === 'supported' && row.descriptor.fieldType === 'secret' && row.descriptor.classification !== 'not_applicable' && row.descriptor.group !== 'internal'));
      services.push({ target: entry.target, fields: projected.fields.length });
    }
    assert.equal(fields, 546); await page.close(); return { services, totalFields: fields };
  });
  await check('GUIDE-02-purpose-brand-explicit-interface-and-cancel', async () => {
    const page = await open(); await page.getByRole('button', { name: '添加服务…', exact: true }).click();
    const multi = catalog.brands.find(brand => catalog.presets.filter(p => p.brandId === brand.brandId && p.allowedPurposes.includes('translation')).length > 1); assert.ok(multi);
    await page.getByRole('dialog').getByRole('button', { name: new RegExp(multi.label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }).first().click();
    assert.ok((await page.getByRole('dialog').getByRole('heading').textContent()).includes('连接方式'));
    const beforeSelection = await page.evaluate(() => window.uiGuided.snapshot().selection); assert.equal(beforeSelection.target.kind, 'http');
    await page.getByRole('button', { name: '返回服务列表', exact: true }).click(); await page.getByRole('combobox', { name: '服务用途', exact: true }).click(); await page.getByRole('option', { name: '句子精读', exact: true }).click();
    const offlineBrand = catalog.brands.find(b => catalog.offlineEngines.some(e => e.brandId === b.brandId)); assert.ok(offlineBrand);
    await page.getByRole('dialog').getByRole('button', { name: new RegExp(offlineBrand.label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }).first().isDisabled().then(value => assert.equal(value, true));
    await page.getByRole('button', { name: '取消', exact: true }).last().click(); assert.deepEqual(await page.evaluate(() => window.uiGuided.snapshot().selection), beforeSelection); await page.close(); return { multiInterfaceBrand: multi.brandId, cancelled: true };
  });
  await check('GUIDE-03-secret-isolation-focus-and-cancel-cleanup', async () => {
    const page = await open(); await choose(page, { kind: 'http', preset: catalog.presets.find(p => p.key.presetId === 'builtin.deepseek').key });
    const password = page.locator('[data-service-field="api_key"] input'); await password.fill('synthetic-secret-never-query');
    const model = page.locator('[data-service-field="model"] input'); await model.fill('deepseek-chat'); await ready(page); assert.equal(await model.evaluate(node => document.activeElement === node), true);
    assert.equal(await password.inputValue(), 'synthetic-secret-never-query');
    assert.equal(JSON.stringify(runtime.requests).includes('synthetic-secret-never-query'), false);
    await page.getByRole('button', { name: '取消', exact: true }).click(); await page.getByRole('status').filter({ hasText: '编辑已关闭' }).waitFor(); const events = await page.evaluate(() => window.uiGuided.snapshot().secretEvents); assert.equal(events.at(-1).length, 0);
    await page.close(); return { requestSecretLeak: false, cleared: true };
  });
  await check('GUIDE-04-defaults-custom-address-google-and-readonly', async () => {
    const page = await open(); await choose(page, { kind: 'http', preset: catalog.presets.find(p => p.key.presetId === 'builtin.deepseek').key });
    await page.getByText('高级设置', { exact: true }).click(); await select(page, 'connection_mode', '使用自定义地址'); await ready(page);
    const endpoint = page.locator('[data-service-field="endpoint"] input'); await endpoint.fill('https://example.invalid/custom'); await ready(page); assert.equal(await endpoint.inputValue(), 'https://example.invalid/custom');
    const google = catalog.presets.find(p => p.binding.protocol.protocolId === 'google_translate_v3'); await choose(page, { kind: 'http', preset: google.key });
    const projected = await page.evaluate(() => window.uiGuided.snapshot().projection); const project = projected.fields.find(row => row.descriptor.id === 'project_id'); assert.ok(project); assert.equal(project.visibility, 'supported'); assert.equal(await page.locator('[data-service-field="project_id"] input').count(), 1);
    const observed = projected.fields.filter(row => row.descriptor.observedOnly && row.visibility === 'supported'); for (const row of observed) assert.equal(await page.locator(`[data-service-field="${row.descriptor.id}"] input`).count(), 0);
    await capture(page, 'google-necessary-1000x720'); await page.close(); return { googleProject: true, observedFields: observed.map(row => row.descriptor.id) };
  });
  await check('GUIDE-05-out-of-order-query-and-error-do-not-win', async () => {
    const page = await open(); await page.evaluate(() => { window.uiGuided.delayNext(350); window.uiGuided.rejectNext(); window.uiGuided.setValues({ model: 'stale-old' }); });
    await page.waitForTimeout(30); const google = catalog.presets.find(p => p.binding.protocol.protocolId === 'google_translate_v3'); await choose(page, { kind: 'http', preset: google.key }); await page.waitForTimeout(450);
    const snapshot = await page.evaluate(() => window.uiGuided.snapshot()); assert.deepEqual(snapshot.projection.target, { kind: 'http', preset: google.key }); assert.equal(snapshot.error, null); assert.equal(snapshot.loading, false); await page.close(); return { staleRejected: true };
  });
  await check('GUIDE-06-keyboard-narrow-and-200-percent-actions-reachable', async () => {
    const facts = [];
    for (const [name, viewport, zoom] of [['normal', { width: 1000, height: 720 }, 1], ['narrow', { width: 760, height: 560 }, 1], ['zoom', { width: 1000, height: 720 }, 2]]) {
      const page = await open(viewport); await page.evaluate(value => document.documentElement.style.zoom = String(value), zoom);
      const save = page.getByRole('button', { name: '保存配置', exact: true }); await save.scrollIntoViewIfNeeded(); await save.focus(); await page.keyboard.press('Enter');
      assert.ok((await page.evaluate(() => window.uiGuided.snapshot().actions)).includes('save'));
      const geometry = await save.evaluate(node => ({ rect: node.getBoundingClientRect().toJSON(), width: innerWidth, height: innerHeight, docOverflow: document.documentElement.scrollWidth > innerWidth }));
      assert.equal(geometry.docOverflow, false); assert.ok(geometry.rect.x >= 0 && geometry.rect.right <= geometry.width && geometry.rect.top >= 0 && geometry.rect.bottom <= geometry.height);
      await capture(page, `editor-${name}`); facts.push({ name, zoom, geometry }); await page.close();
    }
    return facts;
  });
  await check('GUIDE-07-hidden-input-retention-and-secret-erasure', async () => {
    const page=await open(); await choose(page,{kind:'http',preset:catalog.presets.find(p=>p.key.presetId==='builtin.deepseek').key});
    await page.getByText('高级设置',{exact:true}).click(); await select(page,'connection_mode','使用自定义地址'); await ready(page);
    const address=page.locator('[data-service-field="endpoint"] input'); await address.fill('https://example.invalid/retained'); await ready(page);
    const password=page.locator('[data-service-field="api_key"] input'); await password.fill('synthetic-hidden-secret');
    await select(page,'auth_mode','服务器不需要密钥'); await ready(page); assert.equal(await page.locator('[data-service-field="api_key"]').count(),0);
    const events=await page.evaluate(()=>window.uiGuided.snapshot().secretEvents); assert.equal(events.filter(event=>event.id==='api_key').at(-1).length,0);
    await select(page,'auth_mode','使用API Key'); await ready(page); assert.equal(await password.inputValue(),'');
    await select(page,'connection_mode','使用所选服务地址'); await ready(page); assert.equal(await page.locator('[data-service-field="endpoint"]').count(),0);
    await select(page,'connection_mode','使用自定义地址'); await ready(page); assert.equal(await address.inputValue(),'https://example.invalid/retained');
    assert.equal(JSON.stringify(runtime.requests).includes('synthetic-hidden-secret'),false); await page.close();return{ordinaryInputRetained:true,hiddenSecretErased:true};
  });
  await check('GUIDE-08-google-authorization-choice-and-local-action-callbacks',async()=>{
    const page=await open(); const google=catalog.presets.find(p=>p.binding.protocol.protocolId==='google_translate_v3'); await choose(page,{kind:'http',preset:google.key});
    assert.equal(await page.locator('[data-service-field="oauth_login"]').count(),0); assert.equal(await page.locator('[data-service-field="service_account_json"]').count(),0);
    await select(page,'google_auth_method','登录Google授权'); await ready(page); assert.equal(await page.locator('[data-service-field="oauth_login"] button').count(),1); assert.equal(await page.locator('[data-service-field="service_account_json"]').count(),0);
    await page.locator('[data-service-field="oauth_login"] button').click(); assert.ok((await page.evaluate(()=>window.uiGuided.snapshot().actions)).includes('oauth_login'));
    await select(page,'google_auth_method','使用服务账号'); await ready(page); assert.equal(await page.locator('[data-service-field="service_account_json"] button').count(),1); assert.equal(await page.locator('[data-service-field="oauth_login"]').count(),0);
    const argos=catalog.offlineEngines.find(e=>e.engineId==='argos_translate');await choose(page,{kind:'offline',engineId:argos.engineId,contractRevision:argos.contractRevision});
    await page.locator('[data-service-field="installation_path"] button').click();assert.ok((await page.evaluate(()=>window.uiGuided.snapshot().actions)).includes('installation_path'));
    await capture(page,'offline-necessary-1000x720');await page.close();return{credentialBranchesExclusive:true,actionCallbackOnly:true,actualAuthorization:false,actualInstallation:false};
  });
  await check('GUIDE-09-foreign-projection-is-not-adopted',async()=>{
    const page=await open();const before=await page.evaluate(()=>window.uiGuided.snapshot().projection.target);
    await page.evaluate(()=>{window.uiGuided.foreignNext();window.uiGuided.setValues({model:'kept-input'});});await page.waitForFunction(()=>!!window.uiGuided.snapshot().error&&!window.uiGuided.snapshot().loading);
    const state=await page.evaluate(()=>window.uiGuided.snapshot());assert.deepEqual(state.projection.target,before);assert.equal(state.values.model,'kept-input');assert.ok(state.error);await page.close();return{foreignTargetRejected:true,inputKept:true};
  });
  await check('GUIDE-10-modal-keyboard-and-zoom-viewport',async()=>{
    const facts=[];
    for(const [name,viewport,zoom] of [['normal',{width:1000,height:720},1],['narrow',{width:760,height:560},1],['zoom',{width:1000,height:720},2]]){
      const page=await open(viewport);await page.evaluate(value=>document.documentElement.style.zoom=String(value),zoom);
      const trigger=page.getByRole('button',{name:'添加服务…',exact:true});await trigger.focus();await page.keyboard.press('Enter');
      const combo=page.getByRole('combobox',{name:'服务用途',exact:true});await combo.focus();await page.keyboard.press('Space');await page.getByRole('option',{name:'翻译',exact:true}).waitFor();await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');
      assert.ok((await combo.textContent()).includes('句子精读'));await page.waitForFunction(()=>document.activeElement?.getAttribute('aria-label')==='服务用途');assert.equal(await combo.evaluate(node=>document.activeElement===node),true);
      const cancel=page.getByRole('dialog').getByRole('button',{name:'取消',exact:true});await cancel.scrollIntoViewIfNeeded();await cancel.focus();
      const geometry=await cancel.evaluate(node=>({rect:node.getBoundingClientRect().toJSON(),width:innerWidth,height:innerHeight,zoom:getComputedStyle(document.documentElement).zoom,visualViewport:visualViewport?{width:visualViewport.width,height:visualViewport.height,scale:visualViewport.scale,offsetLeft:visualViewport.offsetLeft,offsetTop:visualViewport.offsetTop}:null,modal:node.closest('[data-ui-surface=sheet]')?.getBoundingClientRect().toJSON(),dialog:node.closest('[role=dialog]')?.getBoundingClientRect().toJSON(),documentOverflow:document.documentElement.scrollWidth>innerWidth}));await fs.writeFile(path.join(output,`catalog-${name}-geometry.json`),JSON.stringify(geometry,null,2)+'\n');await capture(page,`catalog-${name}-before-viewport-assert`);assert.equal(geometry.documentOverflow,false);assert.ok(geometry.rect.x>=0&&geometry.rect.right<=geometry.width&&geometry.rect.top>=0&&geometry.rect.bottom<=geometry.height);
      await capture(page,`catalog-${name}`);await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'detached'});await page.waitForFunction(()=>document.activeElement?.textContent==='添加服务…');assert.equal(await trigger.evaluate(node=>document.activeElement===node),true);facts.push({name,zoom,geometry});await page.close();
    }
    return facts;
  });
  await check('GUIDE-11-shared-sheet-appearances-scroll-dismiss-and-nesting',async()=>{
    const facts=[];
    for(const [name,viewport,zoom] of [['normal',{width:1000,height:720},1],['narrow',{width:760,height:560},1],['zoom',{width:1000,height:720},2]]){
      for(const kind of ['selection','confirmation','library','preview']){
        const page=await open(viewport);await page.evaluate(value=>document.documentElement.style.zoom=String(value),zoom);
        const trigger=page.getByRole('button',{name:'共享弹层 '+kind,exact:true});await trigger.scrollIntoViewIfNeeded();await trigger.focus();await page.keyboard.press('Enter');const dialog=page.getByRole('dialog');await dialog.waitFor();
        if(kind==='selection'){const combo=dialog.getByRole('combobox',{name:'回归选项'});await combo.focus();await page.keyboard.press('Space');await page.getByRole('option',{name:'第二项',exact:true}).waitFor();await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');assert.ok((await combo.textContent()).includes('第二项'));}
        const footer=dialog.locator('footer'),header=dialog.locator('header'),body=dialog.locator('[data-ui-scroll="sheet-body"]');const before={header:await header.boundingBox(),footer:await footer.boundingBox()};
        await body.hover();await page.mouse.wheel(0,500);await page.waitForFunction(()=>document.querySelector('[data-ui-scroll="sheet-body"]').scrollTop>0);assert.deepEqual(await header.boundingBox(),before.header);assert.deepEqual(await footer.boundingBox(),before.footer);
        const geometry=await dialog.evaluate(node=>({modal:node.closest('[data-ui-surface=sheet]').getBoundingClientRect().toJSON(),viewport:{width:innerWidth,height:innerHeight},scrolls:[document.documentElement,document.body,document.getElementById('root'),node.closest('[data-ui-surface=sheet]'),node].map(item=>({top:item.scrollTop,left:item.scrollLeft})),body:{top:node.querySelector('[data-ui-scroll="sheet-body"]').scrollTop,client:node.querySelector('[data-ui-scroll="sheet-body"]').clientHeight,scroll:node.querySelector('[data-ui-scroll="sheet-body"]').scrollHeight}}));
        assert.ok(geometry.modal.left>=0&&geometry.modal.top>=0&&geometry.modal.right<=geometry.viewport.width&&geometry.modal.bottom<=geometry.viewport.height);assert.ok(geometry.scrolls.every(item=>item.top===0&&item.left===0));assert.ok(geometry.body.top>0&&geometry.body.scroll>geometry.body.client);
        const finish=dialog.getByRole('button',{name:'完成',exact:true});await finish.focus();assert.equal(await finish.evaluate(node=>document.activeElement===node),true);const rect=await finish.boundingBox();assert.ok(rect.y>=0&&rect.y+rect.height<=viewport.height);
        await capture(page,`shared-${kind}-${name}`);await page.keyboard.press('Escape');await dialog.waitFor({state:'detached'});await page.waitForFunction(label=>document.activeElement?.textContent===label,'共享弹层 '+kind);
        await trigger.click();await page.getByRole('dialog').waitFor();await page.mouse.click(2,2);await page.getByRole('dialog').waitFor({state:'detached'});await page.waitForFunction(label=>document.activeElement?.textContent===label,'共享弹层 '+kind);
        facts.push({name,kind,zoom,geometry,keyboardRestore:true,outsideDismiss:true});await page.close();
      }
    }
    const page=await open();const trigger=page.getByRole('button',{name:'共享弹层 confirmation',exact:true});await trigger.click();const nested=page.getByRole('button',{name:'打开内层确认',exact:true});await nested.click();await page.getByRole('heading',{name:'内层确认',exact:true}).waitFor();await fs.writeFile(path.join(output,'shared-nested-dom.json'),JSON.stringify(await page.locator('[role=dialog]').evaluateAll(nodes=>nodes.map(node=>({html:node.outerHTML,parents:[...function*(element){while(element){yield{tag:element.tagName,hidden:element.getAttribute('aria-hidden'),inert:element.inert};element=element.parentElement}}(node)]}))),null,2)+'\n');await capture(page,'shared-nested-before-assert');const isolation=await page.locator('[role=dialog]').evaluateAll(nodes=>{const parent=nodes.find(node=>node.querySelector('h2')?.textContent==='共享弹层 confirmation'),interactive=parent?[parent,...parent.querySelectorAll('button,input,textarea,select,[tabindex]')]:[];return{active:nodes.filter(node=>!node.closest('[inert],[aria-hidden=true]')).map(node=>node.querySelector('h2')?.textContent),parentExists:!!parent,parentInteractive:interactive.map(node=>({tag:node.tagName,inertAncestor:!!node.closest('[inert]')})),focusedDialog:document.activeElement?.closest('[role=dialog]')?.querySelector('h2')?.textContent}});assert.deepEqual(isolation.active,['内层确认']);assert.equal(isolation.parentExists,true);assert.ok(isolation.parentInteractive.length>0&&isolation.parentInteractive.every(node=>node.inertAncestor));assert.equal(isolation.focusedDialog,'内层确认');const parentFocusAttempt=await page.evaluate(()=>{const parent=[...document.querySelectorAll('[role=dialog]')].find(node=>node.querySelector('h2')?.textContent==='共享弹层 confirmation');parent.querySelector('button').focus();return document.activeElement?.closest('[role=dialog]')?.querySelector('h2')?.textContent});assert.equal(parentFocusAttempt,'内层确认');const tabTrace=[];for(const key of ['Tab','Tab','Tab','Tab','Shift+Tab','Shift+Tab']){await page.keyboard.press(key);const active=await page.evaluate(()=>({dialog:document.activeElement?.closest('[role=dialog]')?.querySelector('h2')?.textContent,label:document.activeElement?.textContent||document.activeElement?.getAttribute('aria-label')}));assert.equal(active.dialog,'内层确认');tabTrace.push({key,...active})}await fs.writeFile(path.join(output,'shared-nested-behavior.json'),JSON.stringify({contract:'Exactly one active modal; underlying DOM may remain inert',isolation,parentFocusAttempt,tabTrace},null,2)+'\n');await page.keyboard.press('Escape');await page.getByRole('heading',{name:'内层确认',exact:true}).waitFor({state:'detached'});await page.waitForFunction(()=>document.activeElement?.textContent==='打开内层确认');assert.equal(await page.getByRole('dialog').count(),1);await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'detached'});await page.waitForFunction(()=>document.activeElement?.textContent==='共享弹层 confirmation');await page.close();return{appearances:facts,nestedDialogEscapeAndRestore:true};
  });
  await check('GUIDE-12-open-sheet-reacts-to-viewport-and-css-zoom',async()=>{
    const page=await open();await page.getByRole('button',{name:'共享弹层 confirmation',exact:true}).click();const measure=()=>page.getByRole('dialog').evaluate(node=>({rect:node.closest('[data-ui-surface=sheet]').getBoundingClientRect().toJSON(),inner:{width:innerWidth,height:innerHeight},cssZoom:getComputedStyle(document.documentElement).zoom,visual:visualViewport?{width:visualViewport.width,height:visualViewport.height,scale:visualViewport.scale}:null}));
    const normal=await measure();await page.evaluate(()=>document.documentElement.style.zoom='2');await page.waitForFunction(()=>{const r=document.querySelector('[data-ui-surface=sheet]').getBoundingClientRect();return r.y>=0&&r.bottom<=innerHeight});const css200=await measure();assert.equal(css200.cssZoom,'2');assert.equal(css200.inner.height,normal.inner.height);assert.equal(css200.visual.scale,1);
    await page.evaluate(()=>document.documentElement.style.zoom='1');await page.setViewportSize({width:500,height:360});await page.waitForFunction(()=>{const r=document.querySelector('[data-ui-surface=sheet]').getBoundingClientRect();return r.x>=0&&r.y>=0&&r.right<=innerWidth&&r.bottom<=innerHeight});const smallerLayoutViewport=await measure();assert.equal(smallerLayoutViewport.inner.width,500);assert.equal(smallerLayoutViewport.cssZoom,'1');
    await page.keyboard.press('Meta+Equal');await page.waitForTimeout(80);const shortcutAttempt=await measure();await capture(page,'shared-resized-layout-viewport');await page.close();return{normal,css200,smallerLayoutViewport,shortcutAttempt,browserChromeZoom:shortcutAttempt.inner.width!==smallerLayoutViewport.inner.width?'observed-layout-resize':'not_available_in_managed_headless_shortcut',pinchZoomClaimed:false};
  });
  assert.deepEqual(browserErrors, []); await verifyBrowserIdentity(browserIdentity);
} catch (error) { checks.push({ id: 'setup-or-completion', status: 'failed', error: error.message, stack: error.stack }); }
finally {
  if (browser) await browser.close();
  if (server) { server.kill('SIGTERM'); await new Promise(resolve => server.once('close', resolve)); }
  if (runtime) await runtime.close();
  const after = await identities(); const sourceStable = JSON.stringify(before) === JSON.stringify(after); if (!sourceStable) checks.push({ id: 'source-stability', status: 'failed' });
  await fs.writeFile(path.join(output, 'vite.log'), serverLog);
  const result = { scope: 'production React components over actual Rust catalogue/form projection; not parent-page, saved configuration, real services or packaged Native', status: checks.length && checks.every(row => row.status === 'passed') ? 'passed' : 'failed', before, after, sourceStable, browserIdentity, checks, screenshots, browserErrors, productAccepted: false, parentPageIntegrated: false };
  await fs.writeFile(path.join(output, 'verification.json'), JSON.stringify(result, null, 2) + '\n'); if (result.status !== 'passed') process.exitCode = 1;
}
