/** Proposal only. Original design render outputs supply expected pixels. The
 * React runner contributes declarative scenario inputs, never expected pixels. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import vm from 'node:vm';
const root = process.cwd(), arg = process.argv.indexOf('--input');
if (arg < 0) throw Error('--input requires a fresh independently rendered v4 proposal directory');
const dir = path.resolve(process.argv[arg + 1]), final = 'docs/design/first-product-v1/acceptance-v4';
try { await fs.access(path.join(dir, 'catalogue.json')); throw Error('Refusing to overwrite an existing reference catalogue'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const require = createRequire(path.join(root, 'package.json')), { PNG } = require('pngjs');
const original = JSON.parse(await fs.readFile(path.join(dir, 'metadata.json'))), hand = JSON.parse(await fs.readFile(path.join(dir, 'hand-authored/metadata.json'))), gaps = JSON.parse(await fs.readFile(path.join(dir, 'reviewed-gaps/metadata.json'))), proposal = JSON.parse(await fs.readFile(path.join(dir, 'v4-proposal.json'))), full = JSON.parse(await fs.readFile(path.join(dir, 'reviewed-full/metadata.json')));
for (const doc of [original, hand, gaps, full]) if (doc.errors?.length) throw Error(`Design producer reported errors: ${doc.errors.join(', ')}`);
for (const [file, expected] of Object.entries(proposal.reviewedSourceIdentities)) if (hash(await fs.readFile(file)) !== expected) throw Error(`Approved source identity changed: ${file}`);
const legacy = JSON.parse(await fs.readFile('docs/design/first-product-v1/acceptance-v3/catalogue.json'));
// This slice is a plain-data contract. Rendering still reads approved design DOM.
const runner = await fs.readFile('apps/desktop/tests/visual/verify-ui.mjs', 'utf8'), start = runner.indexOf('function captureContract(id)'), end = runner.indexOf('// Human-authored design samples', start), designContext = { window: {} };
vm.runInNewContext(await fs.readFile('docs/design/first-product-v1/data.js', 'utf8'), designContext);
const designGame = designContext.window.LunaDesignData.games.find(game => game.id === 'sakura'), designExe = ['C:', 'Program Files', designGame.bottle, designGame.exe].join(String.fromCharCode(92));
const env = { roleSizes: { main: { width: 1040, height: 720 }, settings: { width: 1040, height: 760 }, hud: { width: 660, height: 280 }, learning: { width: 600, height: 670 } }, designGame, designExe };
vm.runInNewContext(runner.slice(start, end) + ';this.contract=captureContract;', env);
const declaredInputs = Object.fromEntries(legacy.states.map(state => [state.id, env.contract(state.id)]));
await fs.writeFile(path.join(dir, 'declared-inputs.json'), JSON.stringify({ schemaVersion: 1, source: 'Shared controlled scenario data only. Product code, CSS, and actual screenshots never supply design pixels.', preferences: { hudBackgroundOpacity: 'Approved reading runtime inline opacity is .82; fallback .72 does not substitute for runtime. Independent word-card supplement retains its declared .72. Product default .70 remains unchanged.' }, states: declaredInputs }, null, 2) + '\n');
const adapterFiles = {};
for (const file of ['reviewed-full/native-full-client.css', 'render.mjs', 'native-content.css', 'hand-authored/index.html', 'hand-authored/supplement.css', 'hand-authored/supplement.js', 'hand-authored/capture.mjs', 'reviewed-gaps/native-component.css']) adapterFiles[`${final}/${file}`] = hash(await fs.readFile(path.join(dir, file)));
for (const file of ['prepare-reviewed-references.mjs', 'prepare-reviewed-gap-references.mjs', 'prepare-reviewed-full-references.mjs', 'build-reviewed-catalogue.mjs']) adapterFiles[`apps/desktop/tests/visual/${file}`] = hash(await fs.readFile(`apps/desktop/tests/visual/${file}`));
for(const entry of await fs.readdir('apps/desktop/tests/visual/reviewed-reference-supplement')){const file='apps/desktop/tests/visual/reviewed-reference-supplement/'+entry;adapterFiles[file]=hash(await fs.readFile(file));}
await fs.writeFile(path.join(dir, 'design-adapter-manifest.json'), JSON.stringify({ schemaVersion: 1, source: 'Independent source-DOM extraction and prior reviewed adapter lineage only. No React, product CSS, actual candidate PNG, screenshot mask or relaxed threshold.', previousLineage: proposal.previousIndependentAdapterLineage, files: adapterFiles }, null, 2) + '\n');
const adapterHash = hash(await fs.readFile(path.join(dir, 'design-adapter-manifest.json')));
const size = async file => { const png = PNG.sync.read(await fs.readFile(path.join(dir, file))); return [png.width, png.height]; };
const canonicalConditions = input => (input.fields??[]).some(field=>field.options)?{...input,canonicalFocus:'neutral'}:input;
const reference = async (file, data) => ({ reference: `${final}/${file}`, referenceSha256: hash(await fs.readFile(path.join(dir, file))), expectedSize: await size(file), deviceScaleFactor: 1, ...data });
const mapScroll = className => className.includes('dialog-body') ? '[data-ui-scroll="sheet-body"]' : className.includes('ls-scroll') ? '[data-ui-scroll="configuration"]' : className.includes('hud-menu') ? '[role="menu"]' : className.includes('hud-reading') ? '[data-ui-scroll="reading"]' : className.includes('main-content') ? '[data-ui-scroll="main-content"]' : '[data-ui-scroll="learning"]';
const states = [], illustrations = [];
for (const row of original.records) {
  const previous = legacy.states.find(state => state.id === row.id), input = declaredInputs[row.id], variants = [];
  if (JSON.stringify(input.viewport) !== JSON.stringify(row.geometry.viewport)) throw Error(`Source and scenario viewport disagree: ${row.id}`);
  for (const file of row.files) variants.push(await reference(file.file, { variantId: file.variant, viewport: input.viewport, selector: previous.variants[0].selector, expectedBounds: file.bounds, scrollTop: file.scrollTop.map(scroll => ({ selector: mapScroll(scroll.className), top: scroll.scrollTop, clientHeight: scroll.clientHeight, scrollHeight: scroll.scrollHeight })), requiredConditions: canonicalConditions(input), details: file.details ?? [], fonts: row.fonts }));
  states.push({ id: row.id, surfaceKind: 'full-window', legacyReference: previous.legacyReference, designSourceHashes: original.sourceHashes, designAdapterHash: adapterHash, dataState: { declaredInput: input, finalDesignState: row.finalDesignState, sourceAdaptations: row.sourceAdaptations }, clientBoundary: row.clientBoundary, nativeExclusions: row.nativeExclusions, underlay: { ...row.underlay, file: `${final}/${row.underlay.file}` }, variants });
}
for (const id of [...new Set(hand.records.map(row => row.id))]) {
  const previous = legacy.states.find(state => state.id === id), input = declaredInputs[id], variants = [];
  for (const row of hand.records.filter(row => row.id === id)) {
    const file = `hand-authored/${row.file}`;
    if (row.state === 'native-window-context') { illustrations.push(await reference(file, { id, kind: 'design-only-native-window-relationship-illustration', not_product_capture: true, not_passed: true, viewport: row.viewport })); continue; }
    variants.push(await reference(file, { variantId: row.state, viewport: row.viewport, selector: '#root', expectedBounds: row.bounds, scrollTop: (row.conditions.scrollRegions ?? []).map(scroll => ({ selector: `[data-ui-scroll="${scroll.selector}"]`, top: scroll.top, clientHeight: scroll.height, scrollHeight: scroll.scrollHeight })), requiredConditions: { ...canonicalConditions(input), ...(id === 'glossary-editing' ? { fields: input.fields.map(field => field.name === '译法 1' ? { ...field, value: row.conditions.terms[0].target } : field) } : {}), viewport: row.viewport, ...(id === 'learning-japanese-linebreak' ? { preferences: { ...input.preferences, sourceFontPt: 18 * row.conditions.fontScale }, sourceFont: `${18 * row.conditions.fontScale}px` } : {}) }, details: row.conditions.details ?? [], designState: row.conditions }));
  }
  const asset = 'design-origin/assets/wallpaper-frame.png';
  states.push({ id, surfaceKind: 'full-window', legacyReference: previous.legacyReference, designSourceHashes: hand.reusedOriginalSources, designAdapterHash: adapterHash, dataState: { declaredInput: input, sourceAdaptations: hand.sourceAdaptations, trace: hand.traces[id] }, clientBoundary: previous.clientBoundary, nativeExclusions: previous.nativeExclusions, underlay: { file: `${final}/${asset}`, sha256: hash(await fs.readFile(path.join(dir, asset))), size: await size(asset), placement: 'center / cover', scope: 'Explicit same controlled native-window material input; not real OS compositor evidence' }, variants });
}
for (const row of full.records) {
  const variants=[];
  const input={windowRole:row.windowRole,viewport:row.viewport,theme:'dark',transparency:'material',...(row.id==='library-populated'?{gameCount:1}:{}),...(row.id.startsWith('settings-')?{settingsCategory:'显示与快捷键'}:{}),...(row.id.startsWith('session-')?{sessionTab:{reading:'正文历史',texts:'全部提取',logs:'Hook 日志'}[row.inputs.tab]}:{})};
  for(const file of row.files) variants.push(await reference('reviewed-full/'+file.file,{variantId:file.variant,viewport:row.viewport,selector:'#root',expectedBounds:file.bounds,scrollTop:file.scrollTop.filter(scroll=>!row.id.startsWith('target-review-')||scroll.className.includes('dialog-body')).map(scroll=>({selector:scroll.className.includes('ls-scroll')?'[data-ui-scroll="preferences"]':scroll.className.includes('dialog-body')?'[data-ui-scroll="sheet-body"]':scroll.className.includes('rw-records')?'[role=log]':'[data-ui-scroll="main-content"]',top:scroll.top,clientHeight:scroll.clientHeight,scrollHeight:scroll.scrollHeight})),requiredConditions:{...input,...(typeof file.sessionFollow==='boolean'?{sessionFollow:file.sessionFollow}:{})}}));
  states.push({id:row.id,surfaceKind:'full-window',requirements:row.requirements,designSourceHashes:full.approvedSourceHashes,designAdapterHash:adapterHash,dataState:{declaredInput:input,fixtureFile:`${final}/reviewed-full/${row.inputFile}`,fixtureSha256:row.inputSha256,previewFrame:row.inputs.frameInput,sourceAdaptations:row.sourceAdaptations},clientBoundary:'Complete native client viewport including navigation, all fields, modal/footer, with continuous scroll variants. No candidate masking or resizing.',nativeExclusions:['OS-owned traffic lights only; native material/capture require packaged verification.'],underlay:{...row.underlay,file:`${final}/underlay-reading.png`,sha256:hash(await fs.readFile(path.join(dir,'underlay-reading.png')))},variants});
}
const componentReferences = [];
for (const row of gaps.records) {
  const input={windowRole:row.kind==='hud'?'hud':'main',viewport:row.viewport,theme:'dark',transparency:'material',...(row.kind==='hud'?{hudOpacity:row.inputs.hud.hudBackgroundOpacity,hudControls:{segmentation:row.inputs.hud.segmentation,ruby:row.inputs.hud.ruby,pos:row.inputs.hud.pos,pin:row.inputs.hud.pin},hudFitHeight:row.viewport.height}:{})};
  const underlay={file:`${final}/underlay-reading.png`,sha256:hash(await fs.readFile(path.join(dir,'underlay-reading.png'))),size:[1600,925],sampleOrigin:row.underlayOrigin,scale:1};
  const variant=await reference(`reviewed-gaps/${row.file}`,{variantId:row.state,viewport:row.viewport,selector:row.kind==='hud'?'#root':row.kind==='select'?'[data-select-panel]':row.kind==='preview'?'[aria-label="运行窗口预览"]':'[data-reviewed-session-surface]',expectedBounds:row.bounds,scrollTop:(row.recordScroll??[]).map(scroll=>({selector:'[role=log]',...scroll})),requiredConditions:input});
  const state={id:row.id,surfaceKind:'component',requirements:row.requirements,designSourceHashes:gaps.approvedSourceHashes,designAdapterHash:adapterHash,dataState:{declaredInput:input,componentInputs:row.inputs,originalBounds:row.originalBounds,componentHost:row.componentHost},clientBoundary:'Complete component in an explicitly declared interactive host viewport; this supplements rather than substitutes for full-window states.',nativeExclusions:['Controlled UI component only; not native capture or game proof'],underlay,variants:[variant]};
  states.push(state);componentReferences.push({...row,reference:variant.reference,referenceSha256:variant.referenceSha256,expectedSize:variant.expectedSize,counterpartStatus:'mandatory_actual_comparison',notPassed:true});
}
// State provenance includes the protected original source/assets as well as the
// sixteen current approved rendering sources. Preserve every declared identity;
// a source must be verified and frozen even when it was not copied into v4.
const stateSourceFiles = {};
for (const state of states) {
  const sources = state.designSourceHashes;
  if (!sources || typeof sources !== 'object' || Array.isArray(sources) || !Object.keys(sources).length) throw Error(`Missing state design provenance: ${state.id}`);
  for (const [file, expected] of Object.entries(sources)) {
    if (!file.startsWith('docs/design/first-product-v1/') || path.posix.normalize(file) !== file || file.includes('\\') || !/^[0-9a-f]{64}$/.test(expected)) throw Error(`Invalid state design source: ${state.id}: ${file}`);
    if (stateSourceFiles[file] && stateSourceFiles[file] !== expected) throw Error(`Conflicting state design source identity: ${file}`);
    if (Object.hasOwn(proposal.reviewedSourceIdentities, file) && proposal.reviewedSourceIdentities[file] !== expected) throw Error(`State design source differs from approved identity: ${file}`);
    if (hash(await fs.readFile(path.resolve(root, file))) !== expected) throw Error(`State design source identity changed: ${state.id}: ${file}`);
    stateSourceFiles[file] = expected;
  }
}
// Complete the already declared no-repeat CSS background over the interactive
// component hosts. The original image stays at 1600x925; pixels outside it use
// the same #020c31 CSS fallback. Expected component/window PNGs never participate.
const fixedComponents = states.filter(state => state.surfaceKind === 'component' && state.underlay?.scale === 1 && Array.isArray(state.underlay.sampleOrigin));
const uncoveredComponents = fixedComponents.filter(state => state.variants.some(variant => state.underlay.sampleOrigin.some((origin, index) => origin + variant.viewport[index ? 'height' : 'width'] > state.underlay.size[index])));
if (uncoveredComponents.length) {
  const sourceRef = `${final}/underlay-reading.png`, sourceFile = path.join(dir, 'underlay-reading.png'), bytes = await fs.readFile(sourceFile), image = PNG.sync.read(bytes);
  if (fixedComponents.some(state => state.underlay.file !== sourceRef || state.underlay.sha256 !== hash(bytes) || JSON.stringify(state.underlay.size) !== JSON.stringify([image.width, image.height]))) throw Error('Component underlay inputs do not share the verified original image');
  const dimensions = ['width', 'height'].map((key, index) => Math.ceil(Math.max(image[index ? 'height' : 'width'], ...fixedComponents.flatMap(state => state.variants.map(variant => state.underlay.sampleOrigin[index] + variant.viewport[key])))));
  const css = `html,body{margin:0;width:100%;height:100%;overflow:hidden;background-color:#020c31}body{background-image:url("data:image/png;base64,${bytes.toString('base64')}");background-size:${image.width}px ${image.height}px;background-position:0px 0px;background-repeat:no-repeat}`;
  const { chromium } = require('playwright-core'), browser = await chromium.launch({ headless: true });
  const outputFile = 'component-common-underlay.png';
  try {
    const page = await browser.newPage({ viewport: { width: dimensions[0], height: dimensions[1] }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
    await page.setContent(`<!doctype html><html><meta charset="utf-8"><style>${css}</style><body></body></html>`);
    await page.evaluate(async src => { const image = new Image(); image.src = src; await image.decode(); await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); }, `data:image/png;base64,${bytes.toString('base64')}`);
    await page.screenshot({ path: path.join(dir, outputFile), animations: 'disabled' });
    const completed = PNG.sync.read(await fs.readFile(path.join(dir, outputFile)));
    if (completed.width !== dimensions[0] || completed.height !== dimensions[1]) throw Error('Independent complete underlay dimensions differ from the derived host extent');
    // Preserve every original input pixel; only the original CSS fallback may
    // occupy the appended area. This checks the new input, not expected PNGs.
    for (let y = 0; y < completed.height; y++) for (let x = 0; x < completed.width; x++) {
      const offset = (y * completed.width + x) * 4, sourceOffset = (y * image.width + x) * 4;
      for (let channel = 0; channel < 4; channel++) {
        const expected = x < image.width && y < image.height ? image.data[sourceOffset + channel] : [2, 12, 49, 255][channel];
        if (completed.data[offset + channel] !== expected) throw Error(`Independent complete underlay changed the original image or CSS fallback: ${x},${y},${channel}`);
      }
    }
    const sha = hash(await fs.readFile(path.join(dir, outputFile)));
    for (const state of uncoveredComponents) state.underlay = { ...state.underlay, file: `${final}/${outputFile}`, sha256: sha, size: dimensions };
    await fs.writeFile(path.join(dir, 'component-underlay-provenance.json'), JSON.stringify({ schemaVersion: 1, kind: 'independent-complete-css-background-input', original: { file: sourceRef, sha256: hash(bytes), size: [image.width, image.height] }, output: { file: `${final}/${outputFile}`, sha256: sha, size: dimensions }, policy: { backgroundColor: '#020c31', backgroundRepeat: 'no-repeat', backgroundPosition: [0, 0], originalImageSizeUnchanged: true, deviceScaleFactor: 1, dimensionRule: 'ceil(max(original extent, every fixed sampleOrigin + complete interactive host viewport))' }, componentHosts: fixedComponents.map(state => ({ id: state.id, sampleOrigin: state.underlay.sampleOrigin, viewport: state.variants.map(variant => variant.viewport) })), extendedStateIds: uncoveredComponents.map(state => state.id), browserVersion: browser.version(), originalPixelsAndFallbackVerified: true, expectedOrProductPixelsUsed: false, originalExpectedPngsModified: false }, null, 2) + '\n');
  } finally { await browser.close(); }
}
const escape = value => String(value).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
await fs.writeFile(path.join(dir, 'index.html'), `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>独立设计参照候选 v4</title><style>body{font:14px/1.8 -apple-system,sans-serif;background:#1c202b;color:#e9e9ef;max-width:1200px;padding:24px;margin:auto}section{border:1px solid #8884;border-radius:13px;margin:24px 0;padding:20px}img{max-width:100%;height:auto}p{color:#b0b2c0}a{color:#c4c5cf}code{overflow-wrap:anywhere}</style><h1>独立设计参照候选 v4</h1><p>等待独立审阅；不是实际产品截图、注册基线或产品验收通过。保留原批准16源37图与零差异阈值。原生窗口控制/材质/真实捕获另做实机验收。舒适布局使用批准源自然172px；正常HUD背景透明度按批准运行态显式输入82%，独立词卡周围HUD保持72%，产品默认70%不变。</p><p><a href="catalogue.json">完整23窗口与14组件状态 / 所有可执行变体</a> · <a href="reviewed-gaps/metadata.json">新增14组件状态与来源</a></p>${states.map(state => `<section><h2>${escape(state.id)}</h2><p>${escape(state.clientBoundary)}</p>${state.variants.map(variant => `<figure><img loading="lazy" src="${variant.reference.slice(final.length + 1)}" alt="${escape(state.id + ' ' + variant.variantId)}"><figcaption>${escape(variant.variantId)} · ${variant.expectedSize.join('×')}</figcaption></figure>`).join('')}</section>`).join('')}<h1>新增已评审组件状态 · 实际组件对照待执行</h1>${componentReferences.map(row => `<section><h2>${escape(row.id)}</h2><p>${escape(row.requirements.join(' · '))} · ${row.expectedSize.join('×')} · ${escape(JSON.stringify(row.inputs))}</p><img loading="lazy" src="reviewed-gaps/${row.file}" alt="${escape(row.id)}"></section>`).join('')}<h2>窗口关系设计说明</h2><p>仅为设计说明，不列为产品像素通过变体。</p>${illustrations.map(item => `<img src="${item.reference.slice(final.length + 1)}" alt="窗口关系说明">`).join('')}</html>`);
await fs.writeFile(path.join(dir, 'geometry-inventory.json'), JSON.stringify({ schemaVersion: 4, status: 'proposal_pending_independent_review', states: states.map(state => ({ id: state.id, surfaceKind: state.surfaceKind, variants: state.variants.map(variant => ({ variantId: variant.variantId, viewport: variant.viewport, selector: variant.selector, expectedBounds: variant.expectedBounds, expectedSize: variant.expectedSize, scrollTop: variant.scrollTop })) })), reviewedComponents: componentReferences }, null, 2) + '\n');
// Close the independent proposal over the final catalogue-stage index/metadata.
// Earlier phase evidence remains in old candidate directories. No self-hash cycles.
proposal.producerStage='catalogue-completed';proposal.files={};
async function closeProposal(base){for(const entry of await fs.readdir(base,{withFileTypes:true})){const file=path.join(base,entry.name);if(entry.isDirectory())await closeProposal(file);else if(!['v4-proposal.json','catalogue.json'].includes(path.relative(dir,file)))proposal.files[path.relative(dir,file)]=hash(await fs.readFile(file));}}
await closeProposal(dir);await fs.writeFile(path.join(dir,'v4-proposal.json'),JSON.stringify(proposal,null,2)+'\n');
const frozenFiles = { ...proposal.reviewedSourceIdentities, ...stateSourceFiles };
async function visit(base) { for (const entry of await fs.readdir(base, { withFileTypes: true })) { const file = path.join(base, entry.name); if (entry.isDirectory()) await visit(file); else if (entry.name !== 'catalogue.json' || path.dirname(file) !== dir) frozenFiles[`${final}/${path.relative(dir, file)}`] = hash(await fs.readFile(file)); } }
await visit(dir);
for (const [file, expected] of Object.entries(adapterFiles)) if (!file.startsWith(final)) frozenFiles[file] = expected;
const catalogue = { schemaVersion: 4, status: 'proposal_pending_independent_review', approvedDesignSources: proposal.reviewedSourceIdentities, pixelmatch: { threshold: .1, includeAA: true, maxRatio: 0 }, designAdapterManifest: `${final}/design-adapter-manifest.json`, states, reviewedComponents: componentReferences, illustrations, nativeBoundaries: legacy.nativeBoundaries, frozenFiles };
await fs.writeFile(path.join(dir, 'catalogue.json'), JSON.stringify(catalogue, null, 2) + '\n');
console.log(JSON.stringify({ status: catalogue.status, states: states.length, variants: states.reduce((count, state) => count + state.variants.length, 0), componentReferences: componentReferences.length, catalogueSha256: hash(await fs.readFile(path.join(dir, 'catalogue.json'))) }));
