/** V2 source-only, coordinate-only client projections. Never native/product approval. */
import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';

const runner = fileURLToPath(import.meta.url), root = path.resolve(path.dirname(runner), '../..');
const relativeRunner = 'scripts/report/capture_current_client_references.mjs';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
assert.equal(process.argv.length, 6); assert.equal(process.argv[2], '--request'); assert.equal(process.argv[4], '--output');
const output = path.resolve(process.argv[5]), requestFile = path.resolve(process.argv[3]);
assert.equal(requestFile, path.join(output, 'request.json'));
assert.ok(path.isAbsolute(process.argv[3]) && path.isAbsolute(process.argv[5]));
async function safe(file, base, regular = true) {
  const real = await fs.realpath(file), resolved = path.resolve(file), boundary = await fs.realpath(base);
  assert.equal(real, resolved, 'Symlink evidence forbidden'); assert.ok(real.startsWith(boundary + path.sep));
  assert.equal((await fs.lstat(real)).isFile(), regular); return real;
}
await safe(output, path.join(root, '.build'), false); await safe(requestFile, output);
assert.deepEqual(await fs.readdir(output), ['request.json'], 'Fresh output required');
const requestBytes = await fs.readFile(requestFile), request = JSON.parse(requestBytes);
const identityCheck = spawnSync('python3', ['-c',
  'import json,sys;from pathlib import Path;r=Path(sys.argv[1]);sys.path.insert(0,str(r/"scripts/quality"));from current_reference_catalogue import client_capture_request,client_c1_capture_request;d=json.loads(Path(sys.argv[2]).read_text());factory=client_c1_capture_request if d.get("schema_version")==3 else client_capture_request;expected=factory(r,[s["state_id"] for s in d["state_descriptors"]],d["authors"],d["execution_nonce"],role=d["client_surface"]["role"],variant=d["client_surface"]["variant"],clock_iso=d["clock_iso"]);assert d==expected,"request differs from code-owned current source/client contract"', root, requestFile], { encoding: 'utf8', cwd: root });
assert.equal(identityCheck.status, 0, identityCheck.stderr);
const require = createRequire(path.join(root, 'package.json')), { chromium } = require('playwright-core');
const surface = request.client_surface, c1 = request.schema_version === 3 ? request.approved_layout : null;
const expectedSources = { ...request.design_sources, ...request.runtime_assets, ...(c1 ? Object.fromEntries(Object.entries(c1.sources).map(([p,r]) => [p,r.sha256]).concat([[c1.approval.path,c1.approval.sha256]])) : {}) };
const c1Css = c1 ? await fs.readFile(path.join(root, c1.contract.styles), 'utf8') : null;
const c1Bootstrap = c1 ? await fs.readFile(path.join(root, c1.contract.bootstrap), 'utf8') : null;
const report = { schema_version: c1 ? 3 : 2, kind: c1 ? 'current-client-reference-generation/v3' : 'current-client-reference-generation/v2',
  ...(c1 ? { approved_layout: c1 } : {}),
  ...Object.fromEntries(['target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets', 'authors', 'execution_nonce'].map(k => [k, request[k]])),
  generator: { path: relativeRunner, sha256: sha(await fs.readFile(runner)) },
  request: { path: 'request.json', sha256: sha(requestBytes) },
  command: [process.execPath, runner, '--request', requestFile, '--output', output], workspace_root: root,
  original_output: output, original_request: requestFile, started_at: new Date().toISOString(), finished_at: null,
  status: 'failed', collection_complete: false, prototype_reference_only: true, native_verified: false,
  product_alignment_approved: false, sources_before: {}, sources_after: {}, source_drift: [],
  page_errors: [], console_errors: [], request_errors: [], external_requests: [], fatal_errors: [], results: [],
  limitations: ['Derived source projection; not an unmodified desktop screenshot.',
    c1 ? 'Explicit user-approved C1 client container CSS/bootstrap; parent content and 61 sources unchanged.' : 'Only the outer role surface coordinates are anchored; source width/height/media/internal layout are unchanged.',
    c1 ? 'Only reviewed C1 roles/sizes/query are allowed; this is an approved container transformation, not coordinate-only.' : 'Geometry mismatch is retained as blocked, never resized.',
    'Native controls, material, real production rendering and product acceptance are separate.'] };
const ref = async relative => { const data = await fs.readFile(path.join(output, relative)); return { path: relative, sha256: sha(data), bytes: data.length }; };
async function json(relative, value) { await fs.writeFile(path.join(output, relative), JSON.stringify(value, null, 2) + '\n'); return ref(relative); }
let fontInventory = [], inventoryError = null;
const CORETEXT_FONT_PROBE = `import Foundation
import CoreText
let name = CommandLine.arguments[1]
let font = CTFontCreateWithName(name as CFString, 18, nil)
let row: [String: Any] = ["requested": name, "resolved": CTFontCopyPostScriptName(font) as String, "family": CTFontCopyFamilyName(font) as String, "url": (CTFontCopyAttribute(font, kCTFontURLAttribute) as? URL)?.path ?? ""]
let data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
print(String(data: data, encoding: .utf8)!)`;
// A code-owned public-API probe, shared with the independent Python replay.
const uiSourceRead = spawnSync('python3', ['-c', 'import sys;sys.path.insert(0,sys.argv[1]);from current_reference_catalogue import CORETEXT_UI_FONT_PROBE;print(CORETEXT_UI_FONT_PROBE,end="")', path.join(root, 'scripts/quality')], { encoding: 'utf8' });
assert.equal(uiSourceRead.status, 0, uiSourceRead.stderr);
const uiProbeSource = uiSourceRead.stdout;
let uiProbeEnvironment, selectedSdk;
const uiProbeCache = new Map();
const fontProbes = new Map();
function actualCommand(command) {
  const result = spawnSync(command[0], command.slice(1), { encoding: 'utf8', timeout: 60000, maxBuffer: 32 * 1024 * 1024 });
  return { command, exit_code: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '', error: result.error?.message ?? null };
}
async function resolveUiFonts(observations) {
  if (!observations.length) return null;
  const inputBytes = Buffer.from(JSON.stringify(observations, null, 2) + '\n'), key = sha(inputBytes);
  if (uiProbeCache.has(key)) return uiProbeCache.get(key);
  const input = `font-probes/ui-input-${key}.json`, rawOutput = `font-probes/ui-result-${key}.json`;
  await fs.writeFile(path.join(output, input), inputBytes, { flag: 'wx' });
  const command = [path.join(output, 'font-probes/ui-font-probe'), path.join(output, input)];
  const execution = actualCommand(command);
  const evidence = await json(rawOutput, { kind: 'coretext-ui-font-execution/v1',
    source: await ref('font-probes/UiFontProbe.swift'), compilation: await ref('font-probes/ui-compilation.json'),
    executable: await ref('font-probes/ui-font-probe'), input: await ref(input),
    environment: uiProbeEnvironment, execution });
  let resolved; try { if (execution.exit_code === 0) resolved = JSON.parse(execution.stdout); } catch { /* Remains a gap. */ }
  const value = { evidence, resolved }; uiProbeCache.set(key, value); return value;
}
async function platformFontFacts(context, page, facts) {
  facts.fonts.platform = []; facts.fonts.files = []; facts.fonts.gaps = []; facts.fonts.observations = [];
  try {
    const cdp = await context.newCDPSession(page);
    try {
      await cdp.send('DOM.enable'); await cdp.send('CSS.enable');
      const document = await cdp.send('DOM.getDocument'), measured = new Map();
      for (const run of facts.text_runs) {
        const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: document.root.nodeId, selector: run.selector });
        assert.ok(nodeId, 'Visible text node missing from platform font measurement');
        const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId });
        for (const font of fonts.filter(font => font.glyphCount > 0)) {
          facts.fonts.observations.push(Object.fromEntries(['selector', 'text', 'font_family', 'font_size', 'font_weight', 'font_style', 'font_stretch', 'font_optical_sizing', 'font_variation_settings'].map(k => [k, run[k]]).concat([['platform_font', font]])));
          const key = font.postScriptName + '\n' + font.familyName, old = measured.get(key);
          measured.set(key, { ...font, glyphCount: font.glyphCount + (old?.glyphCount ?? 0) });
        }
      }
      facts.fonts.platform = [...measured.values()].sort((a, b) => a.postScriptName.localeCompare(b.postScriptName));
    } finally { await cdp.detach(); }
    if (!facts.fonts.platform.length) facts.fonts.gaps.push('effective_platform_fonts_missing');
    if (inventoryError) facts.fonts.gaps.push(inventoryError);
    const uiInputs = facts.fonts.observations.filter(o => !fontInventory.some(file => file.enabled === 'yes' && file.valid === 'yes' && file.typefaces?.some(face => face._name === o.platform_font.postScriptName && face.enabled === 'yes' && face.valid === 'yes')));
    const uiProbe = await resolveUiFonts(uiInputs);
    for (const font of facts.fonts.platform) {
      const candidates = fontInventory.filter(file => file.enabled === 'yes' && file.valid === 'yes'
        && file.typefaces?.some(face => face._name === font.postScriptName && face.enabled === 'yes' && face.valid === 'yes'));
      let originalPath, resolution = 'inventory_unique', coretextProbe = null, uiEvidence = null;
      if (new Set(candidates.map(item => item.path)).size === 1) originalPath = candidates[0].path;
      else if (candidates.length) {
        let probe = fontProbes.get(font.postScriptName);
        if (!probe) {
          const raw = actualCommand(['/usr/bin/xcrun', '--sdk', 'macosx', 'swift', '-sdk', selectedSdk, '-e', CORETEXT_FONT_PROBE, font.postScriptName]);
          const artifact = await json(`font-probes/${sha(font.postScriptName)}.json`, raw);
          let located; try { if (raw.exit_code === 0) located = JSON.parse(raw.stdout); } catch { /* Unavailable remains blocked. */ }
          probe = { artifact, located }; fontProbes.set(font.postScriptName, probe);
        }
        if (probe.located?.requested === font.postScriptName && probe.located?.resolved === font.postScriptName
          && candidates.some(item => item.path === probe.located.url)) {
          originalPath = probe.located.url; resolution = 'coretext_exact'; coretextProbe = probe.artifact;
        }
      } else if (uiProbe?.resolved?.kind === 'coretext-ui-instance-exact/v1') {
        const rows = uiProbe.resolved.results.filter(r => r.source_observation.platform_font.postScriptName === font.postScriptName);
        if (rows.length && rows.every(r => r.status === 'exact_native_match' && r.exact_match?.native?.postscript === font.postScriptName)) {
          const urls = new Set(rows.map(r => r.exact_match.native.url));
          if (urls.size === 1) { originalPath = [...urls][0]; resolution = 'coretext_ui_instance_exact'; uiEvidence = uiProbe.evidence; }
        }
      }
      if (font.isCustomFont || !originalPath) {
        facts.fonts.gaps.push(`resolved_font_bytes_not_attested:${font.postScriptName}`); continue;
      }
      const bytes = await fs.readFile(originalPath), hash = sha(bytes), relative = `font-files/${hash}.font`;
      try { await fs.writeFile(path.join(output, relative), bytes, { flag: 'wx' }); }
      catch (error) { if (error.code !== 'EEXIST') throw error; assert.equal(sha(await fs.readFile(path.join(output, relative))), hash); }
      facts.fonts.files.push({ postscript_name: font.postScriptName, original_path: originalPath, file: await ref(relative), resolution, coretext_probe: coretextProbe, ...(uiEvidence ? { ui_probe: uiEvidence } : {}) });
    }
  } catch (error) { facts.fonts.gaps.push('platform_font_measurement_unavailable:' + error.message); }
}

async function sources() {
  const result = {};
  for (const [relative, expected] of Object.entries(expectedSources)) {
    const data = await fs.readFile(await safe(path.join(root, relative), root));
    result[relative] = sha(data); assert.equal(result[relative], expected, 'Original source changed: ' + relative);
  }
  return result;
}
function factsInBrowser(surface) {
  const node = document.querySelector(surface.selector), content = node?.querySelector(surface.content_selector);
  const box = value => value ? (() => { const r = value.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })() : null;
  if (!node || !content) throw Error('client_surface_missing');
  const visible = value => { const s = getComputedStyle(value), r = value.getBoundingClientRect(); return s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.height > 0; };
  const text = [...node.querySelectorAll('*')].filter(value => visible(value) && Number.parseFloat(getComputedStyle(value).fontSize) > 0 && [...value.childNodes].some(n => n.nodeType === 3 && n.textContent.trim())).map(value => {
    const steps = []; for (let n = value; n !== node; n = n.parentElement) steps.unshift(`${n.tagName.toLowerCase()}:nth-child(${[...n.parentElement.children].indexOf(n) + 1})`);
    const style = getComputedStyle(value); return { selector: surface.selector + ' > ' + steps.join(' > '), text: [...value.childNodes].filter(n => n.nodeType === 3).map(n => n.textContent).join('').trim(), tag: value.tagName,
      font_family: style.fontFamily, font_size: style.fontSize, font_weight: style.fontWeight,
      font_style: style.fontStyle, font_stretch: style.fontStretch, font_optical_sizing: style.fontOpticalSizing, font_variation_settings: style.fontVariationSettings,
      line_height: style.lineHeight, color: style.color, display: style.display, visibility: style.visibility };
  });
  const checks = [...new Map(text.map(run => { const font = `${run.font_weight} ${run.font_size} ${run.font_family}`; return [font + '\n' + run.text, { font, text: run.text, loaded: document.fonts.check(font, run.text) }]; })).values()];
  const forms = [...node.querySelectorAll('input,select,textarea,button')];
  const lexical = selector => [...node.querySelectorAll(selector)].filter(visible).map(value => value.textContent);
  return { viewport: { width: innerWidth, height: innerHeight }, device_pixel_ratio: devicePixelRatio,
    css_zoom: Number.parseFloat(getComputedStyle(document.documentElement).zoom) || 1,
    surface_box: box(node), content_box: box(content), fonts: { status: document.fonts.status, checks }, text_runs: text,
    lexical: { ruby: lexical('ruby'), rt: lexical('rt'), tokens: lexical('[data-token-index]'), pos: lexical('[data-pos],.lp-pos') },
    forms: forms.map((value, index) => ({ index, tag: value.tagName, type: value.type ?? '', value: value.value ?? '', checked: value.checked ?? false,
      selected: value.tagName === 'SELECT' ? [...value.selectedOptions].map(option => option.value) : [], disabled: value.disabled ?? false })),
    focus: forms.includes(document.activeElement) ? forms.indexOf(document.activeElement) : null,
    running_animations: document.getAnimations().filter(a => a.playState === 'running').length,
    capture_scope: 'whole_client', native_attestation: false };
}
function c1LayoutInBrowser(surface) {
  const asBox = node => { const r = node.getBoundingClientRect(); return { x:r.x,y:r.y,width:r.width,height:r.height }; };
  const box = selector => { const node = document.querySelector(selector); return node ? asBox(node) : null; };
  const scrollSelector = surface.role === 'settings' ? '#settings-window .ls-scroll' : '#main-window .main-content';
  const scroll = document.querySelector(scrollSelector), preview = document.querySelector('#main-window .rw-preview-column');
  const result = { viewport:{width:innerWidth,height:innerHeight}, surface:box(surface.selector),
    sidebar:box(surface.role === 'settings' ? '#settings-window .ls-sidebar' : '#main-window .main-sidebar'),
    reading:box('#main-window .guided-reading-layout > :first-child'), preview:preview ? asBox(preview) : null,
    content_scroll: scroll ? { selector:scrollSelector,box:asBox(scroll),client_width:scroll.clientWidth,client_height:scroll.clientHeight,scroll_width:scroll.scrollWidth,scroll_height:scroll.scrollHeight } : null,
    preview_reachability:null };
  if (preview && scroll) {
    const saved = []; for (let node = preview.parentElement; node; node = node.parentElement) saved.push([node,node.scrollLeft,node.scrollTop]);
    const before = {left:scroll.scrollLeft,top:scroll.scrollTop};
    preview.scrollIntoView({block:'nearest',inline:'nearest',behavior:'instant'});
    const firstTarget = asBox(preview), clip = asBox(scroll);
    // scrollIntoView rounds scrollTop to an integer. Actually move the remaining
    // fraction with a legal integer scroll; require exact full visibility after.
    const correction = Math.ceil(Math.max(0, firstTarget.y + firstTarget.height - Math.min(clip.y + scroll.clientHeight, innerHeight)));
    if (correction) scroll.scrollTop += correction;
    const target = asBox(preview);
    const visibleWidth = Math.max(0,Math.min(target.x+target.width,clip.x+scroll.clientWidth,innerWidth)-Math.max(target.x,clip.x,0));
    const visibleHeight = Math.max(0,Math.min(target.y+target.height,clip.y+scroll.clientHeight,innerHeight)-Math.max(target.y,clip.y,0));
    const attempted = {left:scroll.scrollLeft,top:scroll.scrollTop};
    for (const [node,left,top] of saved) {node.scrollLeft=left;node.scrollTop=top;}
    result.preview_reachability = { fully_visible:visibleWidth === target.width && visibleHeight === target.height,
      visible_width:visibleWidth,visible_height:visibleHeight,target,clip,first_target:firstTarget,integer_scroll_correction:correction,before,attempted,
      restored:scroll.scrollLeft === before.left && scroll.scrollTop === before.top };
  }
  return result;
}
async function execute(page, step, origin) {
  const locator = step.selector ? page.locator(step.selector).first() : null;
  switch (step.action) {
    case 'navigate': { assert.ok(Object.hasOwn(expectedSources, step.path)); const url = new URL('/' + step.path, origin); for (const [k, v] of Object.entries(step.query)) url.searchParams.set(k, v); const response = await page.goto(url.href); assert.equal(response.status(), 200); return { url: page.url(), status: 200 }; }
    case 'navigate_scenario': { const url = new URL(page.url()); url.searchParams.set('scenario', step.scenario); const response = await page.goto(url.href); assert.equal(response.status(), 200); return { url: page.url(), status: 200 }; }
    case 'set_viewport': await page.setViewportSize({ width: step.width, height: step.height }); return { viewport: page.viewportSize() };
    case 'fill': await locator.fill(step.value); return { actual: await locator.inputValue() };
    case 'click': await locator.click(); return { clicked_selector: step.selector, matching_count: await page.locator(step.selector).count() };
    case 'select_option': return { selected_values: await locator.selectOption(step.value) };
    case 'press': await page.keyboard.press(step.key); return { key: step.key };
    case 'evaluate_design_api': { assert.equal(step.scope, 'page_local_design_simulation'); assert.equal(step.execution_mode, 'invoke_function_body'); const result = await page.evaluate(async script => { const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor; return await new AsyncFunction(script)(); }, step.script); return { declared_scope: step.scope, result: result ?? null }; }
    case 'assert_visible': await locator.waitFor({ state: 'visible' }); return true;
    case 'assert_absent': { const value = await page.locator(step.selector).count(); assert.equal(value, 0); return value; }
    case 'assert_disabled': { const value = await locator.isDisabled(); assert.equal(value, true); return value; }
    case 'assert_focused': { const value = await locator.evaluate(node => node === document.activeElement); assert.equal(value, true); return value; }
    case 'assert_design_predicate': await page.waitForFunction(expression => (0, eval)(expression) === true, step.expression); return true;
    case 'assert_value': { const value = await locator.inputValue(); assert.equal(value, step.value); return value; }
    case 'assert_text_contains': { const value = await locator.innerText(); assert.ok(value.includes(step.value)); return value; }
    case 'assert_attribute': { const value = await locator.getAttribute(step.name); assert.equal(value, step.value); return value; }
    case 'assert_property': { const value = await locator.evaluate((node, name) => node[name], step.name); assert.deepEqual(value, step.value); return value; }
    default: throw Error('Unsupported source recipe action: ' + step.action);
  }
}
let browser, server, origin;
try {
  report.sources_before = await sources();
  await json('original-source-files.json', expectedSources);
  await fs.mkdir(path.join(output, 'font-files'));
  await fs.mkdir(path.join(output, 'font-probes'));
  const sdk = actualCommand(['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path']);
  assert.equal(sdk.exit_code, 0, 'Actual selected SDK unavailable'); selectedSdk = sdk.stdout.trim();
  const header = path.join(selectedSdk, 'System/Library/Frameworks/CoreText.framework/Headers/CTFont.h');
  const headerBytes = await fs.readFile(header);
  assert.ok(headerBytes.toString().includes('CTFontCreateUIFontForLanguage()'));
  const environment = { sdk_selection: 'macosx', commands: [
    ['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '--version'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path'],
    ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version'], ['/usr/bin/sw_vers']].map(actualCommand),
    sdk_authority: { path: header, sha256: sha(headerBytes), bytes: headerBytes.length } };
  assert.ok(environment.commands.every(row => row.exit_code === 0 && row.stdout.trim()), 'Actual selected SDK identity unavailable');
  uiProbeEnvironment = await json('font-probes/ui-environment.json', environment);
  report.font_probe_environment = await json('font-probe-environment.json', environment);
  await fs.writeFile(path.join(output, 'font-probes/UiFontProbe.swift'), uiProbeSource, { flag: 'wx' });
  const compilation = actualCommand(['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '-sdk', selectedSdk, path.join(output, 'font-probes/UiFontProbe.swift'), '-o', path.join(output, 'font-probes/ui-font-probe')]);
  await json('font-probes/ui-compilation.json', compilation);
  assert.equal(compilation.exit_code, 0, compilation.stderr);
  const inventoryCommand = ['system_profiler', 'SPFontsDataType', '-json'];
  const inventory = spawnSync(inventoryCommand[0], inventoryCommand.slice(1), { encoding: 'utf8', timeout: 60000, maxBuffer: 32 * 1024 * 1024 });
  report.font_inventory = await json('font-inventory.json', { command: inventoryCommand, exit_code: inventory.status,
    stdout: inventory.stdout ?? '', stderr: inventory.stderr ?? '', error: inventory.error?.message ?? null });
  try { assert.equal(inventory.status, 0); fontInventory = JSON.parse(inventory.stdout).SPFontsDataType; assert.ok(Array.isArray(fontInventory)); }
  catch { inventoryError = 'runtime_font_inventory_unavailable'; }
  server = http.createServer(async (req, res) => {
    try {
      const relative = decodeURIComponent(new URL(req.url, 'http://localhost').pathname).slice(1);
      if (c1 && relative === '__approved_client_projection__') { res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }); res.end('<!doctype html><html><head></head><body></body></html>'); return; }
      assert.ok(Object.hasOwn(expectedSources, relative), 'Outside original byte allowlist');
      const bytes = await fs.readFile(await safe(path.join(root, relative), root)); assert.equal(sha(bytes), expectedSources[relative]);
      const mime = { '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript', '.json': 'application/json', '.png': 'image/png', '.svg': 'image/svg+xml' }[path.extname(relative)] ?? 'application/octet-stream';
      res.writeHead(200, { 'content-type': mime, 'cache-control': 'no-store' }); res.end(bytes);
    } catch { res.writeHead(404); res.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ executablePath: chromium.executablePath(), headless: true, args: ['--force-color-profile=srgb'] });
  report.browser = { version: browser.version(), executable: chromium.executablePath(), sha256: sha(await fs.readFile(chromium.executablePath())), playwright: require('playwright-core/package.json').version };
  assert.deepEqual(report.browser, request.browser, 'Actual managed browser differs from authoritative request');
  await fs.mkdir(path.join(output, 'states'));
  for (const item of request.state_descriptors) {
    const descriptor = JSON.parse(item.descriptor_json), directory = `states/${item.state_id}`; await fs.mkdir(path.join(output, directory));
    const row = { state_id: item.state_id, descriptor_sha256: item.descriptor_sha256, capture_scope: 'whole_client', status: 'failed', errors: [], actions: [] };
    const context = await browser.newContext({ viewport: { width: 1600, height: 1100 }, deviceScaleFactor: 1, locale: 'zh-CN', timezoneId: 'Asia/Shanghai', colorScheme: 'dark', reducedMotion: 'reduce', serviceWorkers: 'block' });
    try {
      await context.route('**/*', route => { const url = new URL(route.request().url()); if (!['data:', 'blob:', 'about:'].includes(url.protocol) && url.origin !== origin) { report.external_requests.push(url.href); return route.abort(); } return route.continue(); });
      await context.routeWebSocket('**/*', route => { report.external_requests.push(route.url()); route.close(); });
      await context.addInitScript(now => { const OriginalDate = Date; window.Date = class extends OriginalDate { constructor(...args) { super(...(args.length ? args : [now])); } static now() { return now; } }; }, Date.parse(request.clock_iso));
      const original = await context.newPage(); original.setDefaultTimeout(8000);
      original.on('pageerror', error => report.page_errors.push(error.message)); original.on('console', msg => { if (msg.type() === 'error') report.console_errors.push(msg.text()); });
      original.on('requestfailed', req => report.request_errors.push({ url: req.url(), failure: req.failure() }));
      const effectiveRecipe = descriptor.design_recipe.map(step => step.action === 'navigate' && c1 ? { ...step, query: { ...step.query, ...c1.contract.required_query } } : step);
      if (c1) row.effective_recipe = effectiveRecipe;
      for (let i = 0; i < effectiveRecipe.length; i++) { const step = effectiveRecipe[i]; const actual = await execute(original, step, origin); row.actions.push({ index: i, phase: 'recipe', step, status: 'passed', actual }); }
      await original.evaluate(() => document.fonts.ready); await original.locator(surface.selector).waitFor({ state: 'visible' });
      const before = await original.evaluate(factsInBrowser, surface);
      await platformFontFacts(context, original, before);
      await original.screenshot({ path: path.join(output, directory, 'original.png'), animations: 'disabled' }); row.original_png = await ref(`${directory}/original.png`);
      const captured = await original.evaluate(({ surface, c1Css }) => {
        const originalDom = { url: location.href, document_base_uri: document.baseURI, surface_selector: surface.selector,
          document_html: '<!doctype html>\n' + document.documentElement.outerHTML,
          surface_html: document.querySelector(surface.selector)?.outerHTML };
        const clone = document.documentElement.cloneNode(true), node = clone.querySelector(surface.selector);
        if (!node) throw Error('client_surface_missing');
        // Keep the exact ancestor contexts, but remove desktop sibling surfaces.
        for (let current = node; current.parentElement && current.parentElement.tagName !== 'HTML'; current = current.parentElement) {
          for (const sibling of [...current.parentElement.children]) if (sibling !== current && sibling.tagName !== 'STYLE') sibling.remove();
        }
        clone.querySelectorAll('script').forEach(value => value.remove());
        clone.querySelectorAll('*').forEach(value => [...value.attributes].filter(a => a.name.toLowerCase().startsWith('on')).forEach(a => value.removeAttribute(a.name)));
        clone.querySelectorAll('base').forEach(value => value.remove());
        const base = document.createElement('base'); base.href = document.baseURI; clone.querySelector('head').prepend(base);
        const anchor = document.createElement('style'); anchor.dataset.clientProjection = c1Css === null ? 'coordinate-only-v1' : 'approved-client-c1-v1';
        anchor.textContent = c1Css === null ? `${surface.selector}{position:fixed!important;left:0!important;top:0!important;right:auto!important;bottom:auto!important;transform:none!important}` : c1Css;
        clone.querySelector('head').append(anchor);
        return { originalDom, projectionHtml: '<!doctype html>\n' + clone.outerHTML };
      }, { surface, c1Css });
      const { originalDom, projectionHtml } = captured;
      row.original_dom = await json(`${directory}/original-dom.json`, originalDom);
      const projected = await context.newPage(); projected.on('pageerror', error => report.page_errors.push(error.message));
      projected.on('console', msg => { if (msg.type() === 'error') report.console_errors.push(msg.text()); });
      projected.on('requestfailed', req => report.request_errors.push({ url: req.url(), failure: req.failure() }));
      projected.on('response', res => { if (res.status() >= 400) report.request_errors.push({ url: res.url(), status: res.status() }); });
      await projected.setViewportSize({ width: surface.viewport.width, height: surface.viewport.height });
      if (c1) { const route = new URL('/__approved_client_projection__', origin); route.search = new URL(originalDom.url).search; await projected.goto(route.href); }
      await projected.setContent(projectionHtml, { waitUntil: 'load' });
      if (c1) {
        await projected.evaluate(source => { const execute = new Function(source); execute(); }, c1Bootstrap);
        row.approved_transform = { id: surface.projection, approval: c1.approval, sources: c1.sources,
          bootstrap_sha256: c1.sources[c1.contract.bootstrap].sha256,
          query: Object.fromEntries(new URL(originalDom.url).searchParams),
          root_attributes: await projected.locator('html').evaluate(node => Object.fromEntries([...node.attributes].filter(a => a.name.startsWith('data-client-')).map(a => [a.name, a.value]))) };
      }
      await projected.evaluate(({ selector, forms, focus }) => { const node = document.querySelector(selector), elements = [...node.querySelectorAll('input,select,textarea,button')]; for (const row of forms) { const value = elements[row.index]; if (!value || value.tagName !== row.tag) throw Error('source form identity changed during projection'); if (value.tagName === 'SELECT') [...value.options].forEach(option => option.selected = row.selected.includes(option.value)); else if (value.tagName !== 'BUTTON' && value.value !== row.value) value.value = row.value; if ('checked' in value && value.checked !== row.checked) value.checked = row.checked; } if (focus !== null) elements[focus]?.focus({ preventScroll: true }); }, { selector: surface.selector, forms: before.forms, focus: before.focus });
      await projected.evaluate(() => document.fonts.ready); await projected.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const facts = await projected.evaluate(factsInBrowser, surface);
      await platformFontFacts(context, projected, facts);
      row.client_dom = await json(`${directory}/client-dom.json`, { surface_selector: surface.selector, projection: surface.projection,
        original_surface_html: originalDom.surface_html, surface_html: await projected.locator(surface.selector).evaluate(node => node.outerHTML), document_html: await projected.content() });
      const projectionCommand = ['python3', '-c', 'import json,sys;from pathlib import Path;r=Path(sys.argv[1]);sys.path.insert(0,str(r/"scripts/quality"));from current_reference_catalogue import validate_client_projection,client_c1_authority;surface=json.loads(sys.argv[4]);layout=client_c1_authority(r) if surface.get("projection")=="approved_client_container_c1/v1" else None;validate_client_projection(json.loads(Path(sys.argv[2]).read_text()),json.loads(Path(sys.argv[3]).read_text()),surface,approved_layout={"css":(r/layout["contract"]["styles"]).read_text()} if layout else None)',
        root, path.join(output, row.original_dom.path), path.join(output, row.client_dom.path), JSON.stringify(surface)];
      const projectionCheck = actualCommand(projectionCommand);
      row.projection_check = await json(`${directory}/projection-check.json`, projectionCheck);
      assert.equal(projectionCheck.exit_code, 0, 'Independent coordinate projection check failed: ' + projectionCheck.stderr);
      row.measurements = await json(`${directory}/measurements.json`, { client_surface: surface, projection: surface.projection,
        original_surface_box: before.surface_box, original_viewport: before.viewport, original_facts: before, facts, ...(c1 ? { approved_layout_geometry: await projected.evaluate(c1LayoutInBrowser, surface) } : {}) });
      if (c1) {
        const command = ['python3', '-c', 'import json,sys;from pathlib import Path;r=Path(sys.argv[1]);sys.path.insert(0,str(r/"scripts/quality"));from current_reference_catalogue import validate_c1_layout_geometry,client_c1_authority;m=json.loads(Path(sys.argv[2]).read_text());validate_c1_layout_geometry(m["approved_layout_geometry"],m["client_surface"],client_c1_authority(r))', root, path.join(output, row.measurements.path)];
        const check = actualCommand(command); row.layout_check = await json(`${directory}/layout-check.json`, check);
        assert.equal(check.exit_code, 0, 'Approved C1 actual geometry/reachability failed: ' + check.stderr);
      }
      await projected.screenshot({ path: path.join(output, directory, 'client.png'), animations: 'disabled' }); row.client_png = await ref(`${directory}/client.png`);
      const wanted = { x: 0, y: 0, width: surface.viewport.width, height: surface.viewport.height };
      if (JSON.stringify(facts.surface_box) !== JSON.stringify(wanted) || (!c1 && (before.surface_box.width !== wanted.width || before.surface_box.height !== wanted.height))) {
        row.status = 'blocked'; row.errors.push(c1 ? 'approved_client_geometry_mismatch' : 'client_geometry_mismatch: source dimensions/media rules retained; no resize authorized');
      } else if (facts.fonts.status !== 'loaded' || facts.fonts.checks.some(check => !check.loaded) || facts.fonts.gaps.length) {
        row.status = 'blocked'; row.errors.push('effective_fonts_unavailable');
      } else row.status = 'passed';
    } catch (error) { row.errors.push(error.message); row.stack = error.stack; }
    finally { await context.close(); report.results.push(row); }
  }
  report.sources_after = await sources(); report.source_drift = Object.keys(expectedSources).filter(file => report.sources_after[file] !== report.sources_before[file]);
  if (sha(await fs.readFile(requestFile)) !== report.request.sha256) report.source_drift.push('request.json');
  if (sha(await fs.readFile(runner)) !== report.generator.sha256) report.source_drift.push(relativeRunner);
  if (sha(await fs.readFile(report.browser.executable)) !== report.browser.sha256) report.source_drift.push('managed_browser_executable');
  report.collection_complete = report.results.length === request.state_descriptors.length;
  report.status = report.source_drift.length || report.results.some(row => row.status === 'failed') || ['page_errors', 'console_errors', 'request_errors', 'external_requests'].some(k => report[k].length) ? 'failed' : report.results.some(row => row.status === 'blocked') ? 'blocked' : 'passed';
} catch (error) { report.fatal_errors.push({ message: error.message, stack: error.stack }); }
finally { await browser?.close(); if (server) await new Promise(resolve => server.close(resolve)); report.finished_at = new Date().toISOString(); await json('verification.json', report); console.log(JSON.stringify({ status: report.status, states: report.results.length, output })); }
process.exitCode = report.status === 'passed' ? 0 : 1;
