/** Independent v4 design producer. Reads approved design and prior independent
 * design adapters; never imports product React/CSS or product candidate pixels.
 * Output is a review proposal, never a registered acceptance baseline. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
const root = process.cwd(), option = process.argv.indexOf('--output');
if (option < 0) throw new Error('--output must name a fresh review directory');
const out = path.resolve(process.argv[option + 1]), legacy = path.join(root, 'docs/design/first-product-v1/acceptance-v3');
try { await fs.access(out); throw new Error(`Refusing to overwrite design evidence: ${out}`); } catch (error) { if (error.code !== 'ENOENT') throw error; }
await fs.mkdir(out, { recursive: true });
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const approval = JSON.parse(await fs.readFile('docs/development/design-approval-20261019.json'));
for (const [file, expected] of Object.entries(approval.source_files_sha256)) if (hash(await fs.readFile(file)) !== expected) throw new Error(`Reviewed source changed: ${file}`);
const lineage = {};
for (const file of ['render.mjs', 'native-content.css', 'hand-authored/index.html', 'hand-authored/supplement.css', 'hand-authored/supplement.js', 'hand-authored/capture.mjs']) lineage[`docs/design/first-product-v1/acceptance-v3/${file}`] = hash(await fs.readFile(path.join(legacy, file)));
const adaptations = [
  { id: 'latest-reviewed-sources', basis: 'User-approved exact16 source identities checked before generation; copied separately to design-origin.' },
  { id: 'natural-hud-height', basis: 'Comfortable HUD height is Math.ceil of the approved live-design natural bounds, including26px header/source/translation/footer, never derived from actual React.200% uses the frozen366pt platform work-area cap with complete scroll variants.' },
  { id: 'authored-select-sync', basis: 'Review now requires shared custom Select. Independent DOM options retain values and are upgraded by approved controls.js before serialization; no product component imported.' },
  { id: 'supplemental-hud-common-actions', basis: 'Root approved source-DOM initialization: old independently authored word-card data retained. Surrounding HUD header/footer are extracted only from approved reading scene after renderHUD with default segmentation/ruby/POS/pin true. Earlier raw index HTML omitted these state attributes. No product style/class/color or pixels used; requiredConditions fix each displayed preference.' },
  { id: 'authored-history-label-flow', basis: 'Root independently compared v3/history-earlier.top.png and source render.mjs64 authored label>select; styles.css native select width100%/displayblock originally kept each labelled field in its own row. controls upgrade inserted an inline span inside each label and bypassed the direct-child controls.css21 selector. Native adapter only restores label:block and descendant wrapper width100%; no original source or PNG edited.' },
  { id: 'supplemental-hud-source-runtime', basis: 'Root approved same-unit surrounding HUD source initialization via approved app.render/renderHUD/Learning.sourceHTML: dictionary current source箸 with independent existing dictionary fixture readingはし/名词/筷子; wordcard source新しい原文 with no tokens retains the original independent hand plain1.85 flow: approved renderHUD fallback lp-ruby styling would otherwise add unsupported annotation height. No lexical error or rt is manufactured.72% hand HUD input/card geometries/definitions preserved. No product position/style/PNG consulted; exact input/rendered DOM recorded separately.' },
  { id: 'compact-approved-font-specificity', basis: 'Root independently confirmed approved styles.css compact source16/translation22. The old native-content adapter unconditional #hud-source18/#hud-translation24 had greater specificity; restore only compact16/22 in the independent adapter, without consulting actual pixels or changing approved source.' },
  { id: 'target-preview-contract-identity', basis: 'Root approved projection of the existing registered target-contract-data candidateId design-sample-sakura-window/PID384 into newly added C35 preview metadata. This corrects contradictory offline replay identity without removing any user field or altering original source/v3/PNG.' },
  { id: 'native-content-viewport', basis: 'Prior independently reviewed native client boundary adapter retained. OS traffic lights unpainted; client header/frame/user fields retained. Real compositor/window positions remain separately required.' },
];
let render = await fs.readFile(path.join(legacy, 'render.mjs'), 'utf8');
render = render.replace("path.resolve('.build/acceptance-closure-20261019/visual/registered-candidate-v3-review7')", JSON.stringify(out));
render = render.replace("['previews','acceptance-v3']", "['previews','acceptance-v3','acceptance-v4','candidates']");
render = render.replace(' const [width,height]=sizes[id], [x,y]=coords[id];', ' const [width,declaredHeight]=sizes[id], [x,y]=coords[id]; const naturalBounds=await page.locator(selector).boundingBox(); const height=id==="reading-hud"?Math.ceil(naturalBounds.height):declaredHeight;');
render = render.replace('"display-preferences":[560,344]', '"display-preferences":[560,366]');
render = render.replace('Preserve all original fourteen actions, source, fixed footer and native344px viewport.', 'Preserve the latest reviewed five low-frequency menu actions, all front toolbar actions, source, fixed footer and native366px bounded viewport.');
render = render.replace("   clone.querySelectorAll('.traffic').forEach", "   clone.querySelectorAll('select:not([id])').forEach((select,index)=>{select.id='v4-'+id+'-select-'+index;}); window.LunaControls.upgrade(clone); clone.querySelectorAll('select').forEach(select=>window.LunaControls.sync(select));\n   clone.querySelectorAll('.traffic').forEach");
render = render.replace("   clone.querySelectorAll('.traffic').forEach","   if(id==='target-review'){const metadata=clone.querySelector('.rw-preview-meta>span>small');if(metadata)metadata.textContent='design-sample-sakura-window · PID 384';}\n   clone.querySelectorAll('.traffic').forEach");
render = render.replace("await p.locator('[data-gl-field=\"arch\"]').selectOption('x86');", "await p.evaluate(()=>{const select=document.querySelector('[data-gl-field=\"arch\"]');select.value='x86';select.dispatchEvent(new Event('change',{bubbles:true}));window.LunaControls.sync(select);});");
await fs.writeFile(path.join(out, 'render.mjs'), render);
let nativeCss = await fs.readFile(path.join(legacy, 'native-content.css'), 'utf8');
nativeCss = nativeCss.replace('preserve all fourteen original\n   actions', 'preserve all five current reviewed low-frequency\n   actions');
nativeCss += '\n/* v4 authored history labels retain the original vertical native-select flow after controls upgrade. */\nbody[data-reference-id="history-earlier"] .history-filter label{display:block}body[data-reference-id="history-earlier"] .history-filter .luna-select{width:100%;flex:1}\n';
nativeCss += '\n/* v4: latest reviewed HUD has its own26px header and10px corners. */\n#reference-surface.hud-window{border-radius:10px}.native-design .hud-reading{border-radius:0}\n';
nativeCss += '\n/* Preserve approved compact font semantics against the old native adapter specificity. */\n.native-design .hud-window.compact #hud-source{font-size:calc(16px * var(--reading-scale,1))}.native-design .hud-window.compact #hud-translation{font-size:calc(22px * var(--reading-scale,1))}\n';
await fs.writeFile(path.join(out, 'native-content.css'), nativeCss);
await fs.mkdir(path.join(out, 'hand-authored'), { recursive: true });
for (const file of ['index.html', 'supplement.css', 'supplement.js', 'capture.mjs']) await fs.copyFile(path.join(legacy, 'hand-authored', file), path.join(out, 'hand-authored', file));
let handHtml = await fs.readFile(path.join(out, 'hand-authored/index.html'), 'utf8');
handHtml = handHtml.replace('<link rel="stylesheet" href="supplement.css">', '<link rel="stylesheet" href="../design-origin/controls.css"><link rel="stylesheet" href="../design-origin/review-gaps.css"><link rel="stylesheet" href="supplement.css">');
handHtml = handHtml.replace('</body>', '<script src="../design-origin/controls.js"></script></body>');
await fs.writeFile(path.join(out, 'hand-authored/index.html'), handHtml);
const require=createRequire(path.join(root,'package.json')),{chromium}=require('playwright-core');
const chromeBrowser=await chromium.launch({headless:true});let hudHeader,hudFooter;const supplementSources={};
// Independent common JPEG input encoded from the approved source asset, never from product pixels.
const framePng='docs/design/first-product-v1/assets/sakura-01.png';
{const page=await chromeBrowser.newPage();await page.goto(pathToFileURL(path.join(root,framePng)).href);const encoded=await page.evaluate(async()=>{const image=document.querySelector('img');await image.decode();const canvas=document.createElement('canvas');canvas.width=image.naturalWidth;canvas.height=image.naturalHeight;canvas.getContext('2d').drawImage(image,0,0);return{base64:canvas.toDataURL('image/jpeg',.9).split(',')[1],width:canvas.width,height:canvas.height};});const bytes=Buffer.from(encoded.base64,'base64');await fs.writeFile(path.join(out,'preview-frame.jpg'),bytes);await fs.writeFile(path.join(out,'preview-frame-input.json'),JSON.stringify({file:'preview-frame.jpg',sha256:hash(bytes),encoding:'jpeg',width:encoded.width,height:encoded.height,source:framePng,sourceSha256:hash(await fs.readFile(framePng)),encoder:'Chromium canvas image/jpeg quality0.9; independent of React or product capture',notRealCapture:true},null,2)+'\n');await page.close();}

try{const page=await chromeBrowser.newPage({viewport:{width:1600,height:1100},deviceScaleFactor:1});await page.goto(pathToFileURL(path.join(root,'docs/design/first-product-v1/index.html')).href+'?scenario=reading&theme=dark');await page.waitForFunction(()=>window.LunaDesign?.state.current?.translationStatus==='complete');const chrome=await page.evaluate(()=>({header:document.querySelector('#hud-window .hud-header').outerHTML,footer:document.querySelector('#hud-window .hud-controls').outerHTML,preferences:structuredClone(window.LunaDesign.state.settings),readingOptions:window.LunaSettings.readingOptions(window.LunaDesign.state.settings)}));hudHeader=chrome.header;hudFooter=chrome.footer;await fs.writeFile(path.join(out,'reviewed-hud-chrome.json'),JSON.stringify({source:'Approved reading scene after renderHUD, no product source or pixel input',...chrome},null,2)+'\n');for(const input of [{scenario:'wordcard-native-size',source:'新しい原文。',tokens:[]},{scenario:'dictionary-distinct-headwords',source:'箸',tokens:[{surface:'箸',lemma:'箸',reading:'はし',pos:'名词',meaning:'筷子。'}]}]){const rendered=await page.evaluate(input=>{const app=window.LunaDesign;app.state.current={...app.state.current,id:'independent-'+input.scenario,source:input.source,tokens:input.tokens,translation:'',translationStatus:'complete',language:'ja'};app.state.settings.display.opacity=72;app.render();return{html:document.querySelector('#hud-source').innerHTML,preferences:window.LunaSettings.readingOptions(app.state.settings),input};},input);supplementSources[input.scenario]=rendered;}await fs.writeFile(path.join(out,'reviewed-supplement-hud-sources.json'),JSON.stringify({source:'Root approved independent same-unit renderHUD initialization for surrounding source only; original hand card/definitions/geometries unchanged',approvedJsHashes:Object.fromEntries(Object.entries(approval.source_files_sha256).filter(([file])=>/\/(app|learning|review-gaps)\.js$/.test(file))),scenarios:supplementSources,notProductRuntime:true},null,2)+'\n');}finally{await chromeBrowser.close();}
if(!hudHeader||!hudFooter)throw Error('Approved rendered HUD chrome missing');
let supplement = await fs.readFile(path.join(out, 'hand-authored/supplement.js'), 'utf8');
supplement = `const reviewedHudHeader=${JSON.stringify(hudHeader)},reviewedHudFooter=${JSON.stringify(hudFooter)},reviewedSupplementSources=${JSON.stringify(supplementSources)};\n` + supplement;
supplement = supplement.replace('<p lang="ja">${esc(state.currentSentence)}</p>', '<p lang="ja">${reviewedSupplementSources[scenario]?.input.tokens.length?reviewedSupplementSources[scenario].html:esc(state.currentSentence)}</p>');
supplement = supplement.replace('<select aria-label="选择术语表"', '<select id="v4-term-list" aria-label="选择术语表"');
supplement = supplement.replace('<select aria-label="术语作用范围"', '<select id="v4-term-scope" aria-label="术语作用范围"');
supplement = supplement.replace('<section class="supp-hud" aria-label="当前浮窗原文"><div', '<section class="supp-hud hud-window" aria-label="当前浮窗原文">${reviewedHudHeader}<div');
supplement = supplement.replace('<footer class="supp-hud-controls"><span></span><button aria-label="暂停自动输入">${icon(\'pause\')}</button><button aria-label="更多阅读操作">${icon(\'ellipsis\')}</button></footer>', '${reviewedHudFooter}');
await fs.writeFile(path.join(out, 'hand-authored/supplement.js'), supplement);
let handCss = await fs.readFile(path.join(out, 'hand-authored/supplement.css'), 'utf8');
handCss += '\n/* v4: surrounding HUD uses reviewed chrome; independent card remains unchanged. */\n.supp-hud.hud-window{z-index:0;top:0;left:0;right:0;bottom:0;width:100%;height:100%;max-width:none;border-radius:10px;background:rgba(27,33,48,.72);padding:0}.supp-hud .hud-header,.supp-hud .hud-controls{flex:none}.supp-hud .hud-reading{border-radius:0}.supp-learning .lp-original{line-height:2.2}\n';
await fs.writeFile(path.join(out, 'hand-authored/supplement.css'), handCss);
let capture = await fs.readFile(path.join(out, 'hand-authored/capture.mjs'), 'utf8');
capture = capture.replace("path.resolve('.build/acceptance-closure-20261019/visual/registered-candidate-v3-review7/hand-authored')", JSON.stringify(path.join(out, 'hand-authored')));
capture = capture.replace("await page.getByRole('combobox',{name:'术语作用范围'}).inputValue()", "await page.locator('select[aria-label=\"术语作用范围\"]').inputValue()");
await fs.writeFile(path.join(out, 'hand-authored/capture.mjs'), capture);
async function execute(file, log) { const child=spawn(process.execPath,[file],{cwd:root,stdio:['ignore','pipe','pipe']});let output='';child.stdout.on('data',data=>{output+=data;});child.stderr.on('data',data=>{output+=data;});const code=await new Promise(resolve=>child.on('exit',resolve));await fs.writeFile(path.join(out,log),output);if(code!==0)throw new Error(`Independent design producer failed: ${log}\n${output}`); }
await execute(path.join(out,'render.mjs'),'design-source-render.log');
// Derive the temporary-card horizontal anchor from the approved C16 token DOM,
// not from a product bounding box or candidate screenshot.
const anchorBrowser=await chromium.launch({headless:true});let lexicalAnchor;
try{const page=await anchorBrowser.newPage({viewport:{width:560,height:172},deviceScaleFactor:1});await page.goto(pathToFileURL(path.join(out,'reading-hud.design-source.html')).href);await page.evaluate(()=>document.fonts.ready);lexicalAnchor=await page.locator('#hud-source [data-token-index]').first().boundingBox();if(!lexicalAnchor)throw Error('Approved first token has no anchor');}finally{await anchorBrowser.close();}
await fs.writeFile(path.join(out,'reviewed-word-card-anchor.json'),JSON.stringify({source:'Approved reading DOM with independent native-content adapter, first C16 token border box',sourceFile:'reading-hud.design-source.html',anchor:lexicalAnchor,oldStaticAnchor:19,reason:'Reviewed token margin-left1 now participates in the real source-token boundary; no product position or pixel consulted.'},null,2)+'\n');
await fs.appendFile(path.join(out,'hand-authored/supplement.css'),`\n/* Root-approved: derive from current approved first-token border box. */\n.supp-card{left:${lexicalAnchor.x}px}\n`);
let handCapture=await fs.readFile(path.join(out,'hand-authored/capture.mjs'),'utf8');handCapture=handCapture.replace("change:'word-card-anchor-left19',basis:'Original HUD border1 + comfortable reading horizontal padding18; preserve real lexical anchor. Constrained workarea top/bottom margin8.'", "change:'word-card-anchor-approved-token-dom',basis:'Root approved independent source-DOM derivation recorded in reviewed-word-card-anchor.json; preserve complete card width/height and constrained workarea margins. Replaces obsolete static19 after reviewed C16 adds token margin1. No product coordinates or candidate pixels used.'");await fs.writeFile(path.join(out,'hand-authored/capture.mjs'),handCapture);
await execute(path.join(out,'hand-authored/capture.mjs'),'supplement-render.log');
await fs.cp('apps/desktop/tests/visual/reviewed-reference-supplement',path.join(out,'reviewed-supplement'),{recursive:true,errorOnExist:true});
const gapProducer=path.join(root,'apps/desktop/tests/visual/prepare-reviewed-gap-references.mjs');
{ const child=spawn(process.execPath,[gapProducer,'--output',path.join(out,'reviewed-gaps')],{cwd:root,stdio:['ignore','pipe','pipe']});let output='';child.stdout.on('data',data=>{output+=data;});child.stderr.on('data',data=>{output+=data;});const code=await new Promise(resolve=>child.on('exit',resolve));await fs.writeFile(path.join(out,'reviewed-gaps-render.log'),output);if(code!==0)throw Error(`Reviewed gap producer failed: ${output}`); }
const fullProducer=path.join(root,'apps/desktop/tests/visual/prepare-reviewed-full-references.mjs');
{ const child=spawn(process.execPath,[fullProducer,'--output',path.join(out,'reviewed-full')],{cwd:root,stdio:['ignore','pipe','pipe']});let output='';child.stdout.on('data',data=>{output+=data;});child.stderr.on('data',data=>{output+=data;});const code=await new Promise(resolve=>child.on('exit',resolve));await fs.writeFile(path.join(out,'reviewed-full-render.log'),output);if(code!==0)throw Error(`Reviewed full-window producer failed: ${output}`); }
const full=JSON.parse(await fs.readFile(path.join(out,'reviewed-full/metadata.json')));
const gaps=JSON.parse(await fs.readFile(path.join(out,'reviewed-gaps/metadata.json')));
const main=JSON.parse(await fs.readFile(path.join(out,'metadata.json'))),hand=JSON.parse(await fs.readFile(path.join(out,'hand-authored/metadata.json')));
const mainIds=main.records.map(row=>row.id),handIds=[...new Set(hand.records.map(row=>row.id))];
if(new Set([...mainIds,...handIds]).size!==15)throw new Error('Existing15 window states were not all retained.');
const files={};
async function visit(directory) { for(const entry of await fs.readdir(directory,{withFileTypes:true})){const file=path.join(directory,entry.name);if(entry.isDirectory())await visit(file);else if(entry.isFile())files[path.relative(out,file)]=hash(await fs.readFile(file));} }
await visit(out);
const proposal={schemaVersion:4,status:'proposal_pending_independent_review',notProductAcceptance:true,notRegisteredReference:true,productSourceImported:false,productCandidateImageImported:false,originalApprovedImagesOverwritten:false,zeroDifferencePolicyUnchanged:true,pixelmatch:{threshold:.1,includeAA:true,maxRatio:0},reviewedSourceIdentities:approval.source_files_sha256,previousIndependentAdapterLineage:lineage,sourceAdaptations:adaptations,reviewedFullStateIds:full.records.map(row=>row.id),reviewedFullStates:full.records,reviewedGapStateIds:gaps.records.map(row=>row.id),reviewedGapStates:gaps.records,existingWindowStateIds:[...mainIds,...handIds],existingVariantCount:main.records.reduce((n,row)=>n+row.files.length,0)+hand.records.filter(row=>row.state!=='native-window-context').length,files};
await fs.writeFile(path.join(out,'v4-proposal.json'),JSON.stringify(proposal,null,2)+'\n');
console.log(JSON.stringify({status:proposal.status,output:out,states:15,variants:proposal.existingVariantCount,proposalSha256:hash(await fs.readFile(path.join(out,'v4-proposal.json')))}));
