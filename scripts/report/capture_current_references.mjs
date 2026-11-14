/** Read-only replay of code-owned current design recipes. No product or approval capability. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';

const runner = fileURLToPath(import.meta.url), root = path.resolve(path.dirname(runner), '../..');
const runnerRelative = 'scripts/report/capture_current_references.mjs';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const canonical = value => {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (value && typeof value === 'object') return '{' + Object.keys(value).sort().map(key => JSON.stringify(key) + ':' + canonical(value[key])).join(',') + '}';
  if (typeof value === 'number' && !Number.isFinite(value)) throw Error('Non-finite JSON input');
  const result = JSON.stringify(value); if (result === undefined) throw Error('Non-JSON input'); return result;
};
const exactKeys = (value, keys, label) => assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), label);
const descriptorKeys = ['id', 'scenario', 'source_case_ids', 'state', 'mapping_status', 'design_recipe', 'target', 'fixture', 'variant_measurements'];
const descriptor = row => Object.fromEntries(descriptorKeys.map(key => [key, row[key]]));
async function contained(file, base, { regular = true } = {}) {
  const resolved = path.resolve(file), realBase = await fs.realpath(base), actual = await fs.realpath(resolved);
  assert.ok(actual.startsWith(realBase + path.sep), `Outside ${base}: ${file}`);
  assert.equal(actual, resolved, 'Symlinked input/output paths are forbidden');
  if (regular) assert.equal((await fs.lstat(resolved)).isFile(), true, 'A regular file is required');
  return resolved;
}
function relativeSource(file) {
  assert.equal(typeof file, 'string'); assert.ok(file && !file.includes('\\') && !file.includes('\0') && !path.isAbsolute(file));
  assert.ok(file.split('/').every(part => part && part !== '.' && part !== '..'), 'Canonical relative source required'); return file;
}
async function readSourceMap(expected, rawFile) {
  const identities = {}, raw = {};
  for (const [relative, expectedSha] of Object.entries(expected).sort(([a], [b]) => a.localeCompare(b))) {
    relativeSource(relative); assert.match(expectedSha, /^[0-9a-f]{64}$/);
    const file = await contained(path.join(root, relative), root), bytes = await fs.readFile(file), actual = sha(bytes);
    assert.equal(actual, expectedSha, `Original design/runtime bytes drift: ${relative}`); identities[relative] = actual;
    if (rawFile) { const encoding = /\.(?:html|css|js|json|md|txt)$/.test(relative) ? 'utf8' : 'base64'; raw[relative] = { sha256: actual, bytes: bytes.length, encoding, content: bytes.toString(encoding) }; }
  }
  if (rawFile) await fs.writeFile(rawFile, JSON.stringify(raw, null, 2) + '\n');
  return identities;
}
async function sourceIdentities(expected) {
  const result = {};
  for (const file of Object.keys(expected).sort()) { try { result[file] = sha(await fs.readFile(await contained(path.join(root, relativeSource(file)), root))); } catch { result[file] = null; } }
  return result;
}

assert.equal(process.argv.length, 6, 'Fixed CLI: node <runner> --request <absoluteOut/request.json> --output <absoluteOut>');
assert.equal(process.argv[2], '--request'); assert.equal(process.argv[4], '--output');
assert.ok(path.isAbsolute(process.argv[3]) && path.isAbsolute(process.argv[5]), 'Absolute request/output required');
const out = path.resolve(process.argv[5]), requestPath = path.resolve(process.argv[3]);
assert.equal(requestPath, path.join(out, 'request.json'));
await contained(out, path.join(root, '.build'), { regular: false }); assert.equal((await fs.lstat(out)).isDirectory(), true);
assert.deepEqual((await fs.readdir(out)).sort(), ['request.json'], 'Output must be fresh except its immutable request.json');
await contained(requestPath, out);
const requestBytes = await fs.readFile(requestPath), request = JSON.parse(requestBytes);
const report = {
  schema_version: 1, kind: 'current-design-reference-generation/v1',
  target_id: request.target_id, composition_id: request.composition_id, design_closure_sha256: request.design_closure_sha256,
  design_sources: request.design_sources, runtime_assets: request.runtime_assets, authors: request.authors,
  execution_nonce: request.execution_nonce, clock_iso: request.clock_iso,
  request: { path: 'request.json', sha256: sha(requestBytes) },
  command: [process.execPath, runner, '--request', requestPath, '--output', out], workspace_root: root,
  original_output: out, original_request: requestPath,
  started_at: new Date().toISOString(), finished_at: null, status: 'failed', collection_complete: false,
  prototype_reference_only: true, product_alignment_approved: false, native_verified: false,
  sources_before: {}, sources_after: {}, source_drift: [], page_errors: [], external_requests: [], console_errors: [], request_errors: [],
  fatal_errors: [], results: [], browser: null,
  environment: { node: process.version, platform: process.platform, architecture: process.arch, os_release: os.release(), cwd: process.cwd(), process_versions: process.versions },
  generator: { path: runnerRelative, sha256: sha(await fs.readFile(runner)) },
  limitations: ['Original design DOM only; production components/CSS are never loaded.', 'Capture completion and passed source assertions grant no reference review, product alignment, packaged/native or game approval.', 'Original target/full PNGs are never masked, resized or replaced; absent targets remain failed with no fabricated target image.'],
};
const require = createRequire(path.join(root, 'package.json')), { chromium } = require('playwright-core'), { PNG } = require('pngjs');
let browser, server, origin;
const served = [];
const artifact = async relative => { const bytes = await fs.readFile(path.join(out, relative)); return { path: relative, sha256: sha(bytes), bytes: bytes.length }; };
async function writeJson(relative, data) { await fs.writeFile(path.join(out, relative), JSON.stringify(data, null, 2) + '\n'); return artifact(relative); }
const sourceFile = 'docs/development/all-page-design-coverage.json';
const assertionActions = new Set(['assert_visible', 'assert_design_predicate', 'assert_value', 'assert_text_contains', 'assert_property', 'assert_focused', 'assert_attribute', 'assert_disabled', 'assert_absent']);
const actionKeys = {
  navigate: ['action', 'path', 'query'], navigate_scenario: ['action', 'scenario'], set_viewport: ['action', 'width', 'height'],
  fill: ['action', 'selector', 'value'], click: ['action', 'selector'], select_option: ['action', 'selector', 'value'], press: ['action', 'key'],
  evaluate_design_api: ['action', 'script', 'scope', 'execution_mode'],
  assert_visible: ['action', 'selector'], assert_design_predicate: ['action', 'expression'], assert_value: ['action', 'selector', 'value'],
  assert_text_contains: ['action', 'selector', 'value'], assert_property: ['action', 'selector', 'name', 'value'], assert_focused: ['action', 'selector'],
  assert_attribute: ['action', 'selector', 'name', 'value'], assert_disabled: ['action', 'selector'], assert_absent: ['action', 'selector'],
};
async function observations(page) {
  return page.evaluate(() => ({ url: location.href, theme: document.documentElement.dataset.theme ?? null, zoom: Number.parseFloat(getComputedStyle(document.documentElement).zoom) || 1,
    viewport: { width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio },
    font: { family: getComputedStyle(document.body).fontFamily, size: getComputedStyle(document.body).fontSize },
    scroll: { x: scrollX, y: scrollY },
    designState: window.LunaDesign?.state ?? null, historyState: window.LunaHistoryDesign?.getState?.() ?? null,
    browser: { userAgent: navigator.userAgent, language: navigator.language, platform: navigator.platform, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone },
  }));
}
async function readGeometry(page, selector) {
  const viewport = page.viewportSize();
  const measured = await page.evaluate(selector => {
    const nodes = [...document.querySelectorAll(selector)], node = nodes[0];
    const rectangle = value => { const r = value.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
    if (!node) return { target_box: null, matches: 0, clip: null, scroll_regions: [] };
    const box = rectangle(node), intersection = { x: Math.max(0, box.x), y: Math.max(0, box.y), width: Math.max(0, Math.min(innerWidth, box.x + box.width) - Math.max(0, box.x)), height: Math.max(0, Math.min(innerHeight, box.y + box.height) - Math.max(0, box.y)) };
    const computed = getComputedStyle(node);
    return { target_box: box, matches: nodes.length, target_selection: 'first DOM match, as document.querySelector; no arbitrary visible replacement',
      match_boxes: nodes.map(rectangle),
      clip: { within_viewport: box.x >= 0 && box.y >= 0 && box.x + box.width <= innerWidth && box.y + box.height <= innerHeight,
        intersection, overflow: { left: Math.max(0, -box.x), top: Math.max(0, -box.y), right: Math.max(0, box.x + box.width - innerWidth), bottom: Math.max(0, box.y + box.height - innerHeight) },
        document_scroll: { x: scrollX, y: scrollY },
        ancestors: [...function* () { for (let parent = node.parentElement; parent; parent = parent.parentElement) yield parent; }()].filter(parent => ['hidden', 'clip', 'auto', 'scroll'].some(value => [getComputedStyle(parent).overflowX, getComputedStyle(parent).overflowY].includes(value))).map(parent => ({ tag: parent.tagName, id: parent.id, box: rectangle(parent), overflowX: getComputedStyle(parent).overflowX, overflowY: getComputedStyle(parent).overflowY, scrollTop: parent.scrollTop, scrollLeft: parent.scrollLeft, clientWidth: parent.clientWidth, clientHeight: parent.clientHeight, scrollWidth: parent.scrollWidth, scrollHeight: parent.scrollHeight })) },
      computed: { display: computed.display, visibility: computed.visibility, opacity: computed.opacity, background: computed.backgroundColor, color: computed.color, fontFamily: computed.fontFamily, fontSize: computed.fontSize, fontWeight: computed.fontWeight, lineHeight: computed.lineHeight, backdropFilter: computed.backdropFilter, borderRadius: computed.borderRadius },
      scroll_regions: [node, ...node.querySelectorAll('*')].filter(value => value.scrollHeight > value.clientHeight || value.scrollWidth > value.clientWidth).map(value => ({ tag: value.tagName, id: value.id, top: value.scrollTop, left: value.scrollLeft, clientHeight: value.clientHeight, scrollHeight: value.scrollHeight, clientWidth: value.clientWidth, scrollWidth: value.scrollWidth })),
    };
  }, selector);
  return { viewport: { ...viewport, deviceScaleFactor: 1 }, ...measured };
}
async function writeSourceDom(page, sid, selector) {
  const target = page.locator(selector).first();
  return writeJson(`states/${sid}/source-dom.json`, { url: page.url(), selector, document_html: await page.content(), target_html: await target.count() ? await target.evaluate(node => node.outerHTML) : null });
}
async function readAssertionActual(page, step) {
  const first = step.selector ? page.locator(step.selector).first() : null;
  if (step.action === 'assert_design_predicate') return page.evaluate(expression => (0, eval)(expression), step.expression);
  if (step.action === 'assert_absent') return page.locator(step.selector).count();
  if (step.action === 'assert_visible') return first.isVisible();
  if (!await first.count()) return null;
  switch (step.action) {
    case 'assert_disabled': return first.isDisabled();
    case 'assert_focused': return first.evaluate(node => node === document.activeElement);
    case 'assert_text_contains': return first.innerText();
    case 'assert_attribute': return first.getAttribute(step.name);
    case 'assert_property': return first.evaluate((node, name) => node[name], step.name);
    case 'assert_value': return first.inputValue();
    default: throw Error('Unknown actual measurement: ' + step.action);
  }
}
async function executeAssertion(page, step) {
  const first = step.selector ? page.locator(step.selector).first() : null;
  switch (step.action) {
    case 'assert_visible': await first.waitFor({ state: 'visible' }); return await first.isVisible();
    case 'assert_absent': { const count = await page.locator(step.selector).count(); assert.equal(count, 0); return count; }
    case 'assert_design_predicate': {
      await page.waitForFunction(expression => (0, eval)(expression) === true, step.expression);
      const actual = await page.evaluate(expression => (0, eval)(expression), step.expression); assert.equal(actual, true); return actual;
    }
    case 'assert_value': await page.waitForFunction(({ selector, expected }) => document.querySelector(selector)?.value === expected, { selector: step.selector, expected: step.value }); return await first.inputValue();
    case 'assert_text_contains': {
      await page.waitForFunction(({ selector, expected }) => document.querySelector(selector)?.innerText.includes(expected), { selector: step.selector, expected: step.value });
      const actual = await first.innerText(); assert.ok(actual.includes(step.value)); return actual;
    }
    case 'assert_property': { await page.waitForFunction(({ selector, name, expected }) => document.querySelector(selector)?.[name] === expected, { selector: step.selector, name: step.name, expected: step.value }); const actual = await first.evaluate((node, name) => node[name], step.name); assert.deepEqual(actual, step.value); return actual; }
    case 'assert_attribute': await page.waitForFunction(({ selector, name, expected }) => document.querySelector(selector)?.getAttribute(name) === expected, { selector: step.selector, name: step.name, expected: step.value }); return await first.getAttribute(step.name);
    case 'assert_focused': await page.waitForFunction(selector => document.querySelector(selector) === document.activeElement, step.selector); return await first.evaluate(node => node === document.activeElement);
    case 'assert_disabled': { const actual = await first.isDisabled(); assert.equal(actual, true); return actual; }
    default: throw Error(`Unknown assertion action: ${step.action}`);
  }
}
async function executeAction(page, step, expectedSources) {
  exactKeys(step, actionKeys[step.action] ?? [], 'Unknown or malformed recipe action: ' + step.action);
  const locator = step.selector ? page.locator(step.selector).first() : null;
  switch (step.action) {
    case 'navigate': {
      const file = relativeSource(step.path); assert.ok(Object.hasOwn(expectedSources, file) && file.endsWith('.html'), 'Navigation must use an original declared design HTML');
      const url = new URL('/' + file, origin); for (const [key, value] of Object.entries(step.query)) { assert.equal(typeof value, 'string'); url.searchParams.set(key, value); }
      const response = await page.goto(url.href, { waitUntil: 'load' }); assert.equal(response.status(), 200); return { url: page.url(), status: response.status() };
    }
    case 'navigate_scenario': { const url = new URL(page.url()); assert.equal(url.origin, origin); url.searchParams.set('scenario', step.scenario); const response = await page.goto(url.href, { waitUntil: 'load' }); assert.equal(response.status(), 200); return { url: page.url(), status: response.status() }; }
    case 'set_viewport': assert.ok(Number.isInteger(step.width) && step.width > 0 && step.width <= 4096 && Number.isInteger(step.height) && step.height > 0 && step.height <= 4096); await page.setViewportSize({ width: step.width, height: step.height }); return { viewport: page.viewportSize() };
    case 'fill': await locator.fill(step.value); return { actual: await locator.inputValue() };
    case 'click': await locator.click(); return { clicked_selector: step.selector, matching_count: await page.locator(step.selector).count() };
    case 'select_option': return { selected_values: await locator.selectOption(step.value) };
    case 'press': await page.keyboard.press(step.key); return { key: step.key };
    case 'evaluate_design_api': {
      assert.equal(step.scope, 'page_local_design_simulation'); assert.equal(step.execution_mode, 'invoke_function_body');
      // Only an exact code-owned descriptor script can reach this branch.
      const result = await page.evaluate(async script => { const invoke = Object.getPrototypeOf(async function () {}).constructor; return await new invoke(script)(); }, step.script);
      return { declared_scope: step.scope, result: result ?? null };
    }
    default: throw Error('Unknown recipe action: ' + step.action);
  }
}

try {
  exactKeys(request, ['schema_version', 'kind', 'target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets', 'execution_nonce', 'authors', 'clock_iso', 'generator', 'state_descriptors'], 'Exact capture request schema required');
  assert.equal(request.schema_version, 1); assert.equal(request.kind, 'current-design-reference-request/v1'); assert.match(request.execution_nonce, /^[0-9a-f]{32}$/);
  assert.ok(Array.isArray(request.authors) && request.authors.length && new Set(request.authors).size === request.authors.length && request.authors.every(author => typeof author === 'string' && author.trim() === author && author.length > 0 && author.length <= 240 && !/[\x00-\x1f\x7f]/.test(author)));
  assert.deepEqual(request.generator, report.generator);
  const clock = Date.parse(request.clock_iso); assert.ok(Number.isFinite(clock) && /(?:Z|[+-]\d\d:\d\d)$/.test(request.clock_iso));
  assert.ok(Array.isArray(request.state_descriptors) && request.state_descriptors.length);
  const expectedSources = { ...request.design_sources, ...request.runtime_assets };
  for (const [file, hash] of Object.entries(request.runtime_assets)) if (Object.hasOwn(request.design_sources, file)) assert.equal(hash, request.design_sources[file], 'Conflicting original source identity');
  await fs.mkdir(path.join(out, 'inputs')); await fs.mkdir(path.join(out, 'states'));
  report.sources_before = await readSourceMap(expectedSources, path.join(out, 'inputs/source-files.json'));
  const documentBytes = await fs.readFile(path.join(root, sourceFile)), coverage = JSON.parse(documentBytes);
  report.descriptor_catalogue = { path: sourceFile, sha256: sha(documentBytes) }; await fs.writeFile(path.join(out, 'inputs/all-page-design-coverage.json'), documentBytes);
  const known = new Map(coverage.pages.flatMap(page => page.states).map(row => [row.id, descriptor(row)]));
  const requests = [], unique = new Set();
  for (const item of request.state_descriptors) {
    exactKeys(item, ['state_id', 'descriptor_json', 'descriptor_sha256'], 'Exact state descriptor required'); assert.match(item.state_id, /^[A-Za-z0-9][A-Za-z0-9._-]*$/); assert.equal(unique.has(item.state_id), false); unique.add(item.state_id);
    assert.equal(sha(Buffer.from(item.descriptor_json)), item.descriptor_sha256, 'Descriptor SHA before parsing');
    const parsed = JSON.parse(item.descriptor_json); assert.equal(canonical(parsed), item.descriptor_json, 'Canonical descriptor JSON required');
    assert.equal(parsed.id, item.state_id); assert.ok(['mapped_recipe', 'mapped_absent'].includes(parsed.mapping_status)); assert.equal(canonical(known.get(item.state_id)), item.descriptor_json, 'Descriptor must be the actual code-owned all-page row');
    assert.ok(Array.isArray(parsed.design_recipe) && parsed.design_recipe.length); assert.ok(parsed.fixture && parsed.fixture.kind === 'page_local_design_simulation');
    for (const step of parsed.design_recipe) { assert.ok(Object.hasOwn(actionKeys, step.action), 'Unknown design recipe action: ' + step.action); exactKeys(step, actionKeys[step.action], 'Exact declared recipe step'); }
    requests.push({ item, descriptor: parsed });
  }
  server = http.createServer(async (req, res) => {
    let relative;
    try {
      relative = decodeURIComponent(new URL(req.url, 'http://localhost').pathname).slice(1); relativeSource(relative);
      assert.ok(Object.hasOwn(expectedSources, relative), 'Only declared original design/runtime resources can be served');
      const bytes = await fs.readFile(await contained(path.join(root, relative), root)); assert.equal(sha(bytes), expectedSources[relative]);
      const extension = path.extname(relative).toLowerCase(), type = ({ '.html': 'text/html; charset=utf-8', '.js': 'application/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' })[extension] ?? 'application/octet-stream';
      served.push({ path: relative, sha256: sha(bytes), bytes: bytes.length, status: 200 }); res.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' }); res.end(bytes);
    } catch (error) { served.push({ path: relative ?? req.url, status: 404, error: error.message }); res.writeHead(404); res.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve)); origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ headless: true, args: ['--force-color-profile=srgb'] });
  report.browser = { version: browser.version(), playwright_version: require('playwright-core/package.json').version, executable: chromium.executablePath(), engine: 'Playwright-managed pinned Chromium', default_viewport: { width: 1600, height: 1100, deviceScaleFactor: 1 }, locale: 'zh-CN', timezone: 'Asia/Shanghai', reduced_motion: 'reduce' };
  for (const input of requests) {
    const sid = input.item.state_id, stateOut = path.join(out, 'states', sid); await fs.mkdir(stateOut);
    const row = { state_id: sid, descriptor_sha256: input.item.descriptor_sha256, status: 'failed', errors: [], assertion_count: 0, skipped_assertions: 0, artifacts: { target_png: null, full_png: null, geometry: null, assertions: null }, pixel_sizes: { target_png: null, full_png: null }, actions: [], source_dom: null, inputs: null };
    const transcript = [], localErrors = [], localExternal = [], localConsole = [], localRequests = [];
    const context = await browser.newContext({ viewport: { width: 1600, height: 1100 }, deviceScaleFactor: 1, locale: 'zh-CN', timezoneId: 'Asia/Shanghai', colorScheme: 'dark', reducedMotion: 'reduce', serviceWorkers: 'block' });
    let page, failedIndex = null;
    try {
      await context.route('**/*', async route => { const url = new URL(route.request().url()); if (!['data:', 'blob:', 'about:'].includes(url.protocol) && url.origin !== origin) { const event = { state_id: sid, url: url.href, method: route.request().method() }; localExternal.push(event); report.external_requests.push(event); return route.abort('blockedbyclient'); } return route.continue(); });
      await context.routeWebSocket('**/*', route => { const event = { state_id: sid, url: route.url(), protocol: 'websocket' }; localExternal.push(event); report.external_requests.push(event); route.close({ code: 1008, reason: 'Static design replay forbids WebSocket transport' }); });
      await context.addInitScript(now => { const OriginalDate = Date; window.Date = class extends OriginalDate { constructor(...args) { super(...(args.length ? args : [now])); } static now() { return now; } }; }, clock);
      page = await context.newPage(); page.setDefaultTimeout(8000); page.setDefaultNavigationTimeout(15000);
      page.on('pageerror', error => { const item = { state_id: sid, message: error.message, stack: error.stack }; localErrors.push(item); report.page_errors.push(item); });
      page.on('console', message => { if (message.type() === 'error') { const item = { state_id: sid, text: message.text(), location: message.location() }; localConsole.push(item); report.console_errors.push(item); } });
      page.on('requestfailed', request => { const item = { state_id: sid, url: request.url(), failure: request.failure() }; localRequests.push(item); report.request_errors.push(item); });
      page.on('response', response => { if (response.status() >= 400) { const item = { state_id: sid, url: response.url(), status: response.status() }; localRequests.push(item); report.request_errors.push(item); } });
      const recipe = input.descriptor.design_recipe;
      for (let index = 0; index < recipe.length; index++) {
        const step = recipe[index];
        try {
          if (assertionActions.has(step.action)) { const actual = await executeAssertion(page, step); const item = { ...step, step_index: index, status: 'passed', actual }; transcript.push(item); row.actions.push({ index, phase: 'recipe', step, status: 'passed', actual }); }
          else row.actions.push({ index, phase: 'recipe', step, status: 'passed', actual: await executeAction(page, step, expectedSources) });
        } catch (error) {
          const item = { index, phase: 'recipe', step, status: 'failed', error: error.message, stack: error.stack }; row.actions.push(item); if (assertionActions.has(step.action)) { let actual = null; try { actual = await readAssertionActual(page, step); } catch {} transcript.push({ ...step, step_index: index, status: 'failed', actual, error: error.message }); } row.errors.push('Recipe step ' + index + ': ' + error.message); failedIndex = index; break;
        }
      }
      if (failedIndex !== null) for (let index = failedIndex + 1; index < recipe.length; index++) { row.actions.push({ index, phase: 'recipe', step: recipe[index], status: 'not_run', reason: 'Earlier recipe step failed' }); if (assertionActions.has(recipe[index].action)) row.skipped_assertions++; }
      const selector = input.descriptor.target.selector; assert.equal(input.descriptor.target.visibility, 'required_visible');
      const beforeCapture = await observations(page), beforeGeometry = await readGeometry(page, selector);
      await page.evaluate(() => document.fonts.ready);
      const target = page.locator(selector).first();
      if (await target.count() && await target.isVisible()) {
        const bounds = await target.boundingBox(); assert.ok(bounds && bounds.width > 0 && bounds.height > 0 && bounds.width <= 4096 && bounds.height <= 4096, 'Original target must fit the bounded PNG decoder; never resize it');
        const targetFile = `states/${sid}/target.png`; await target.screenshot({ path: path.join(out, targetFile), animations: 'disabled' }); row.artifacts.target_png = await artifact(targetFile); const decoded = PNG.sync.read(await fs.readFile(path.join(out, targetFile))); row.pixel_sizes.target_png = [decoded.width, decoded.height];
      } else row.errors.push('Declared target is absent or hidden; no substitute target PNG was generated');
      const fullFile = `states/${sid}/full.png`; await page.screenshot({ path: path.join(out, fullFile), animations: 'disabled', fullPage: false }); row.artifacts.full_png = await artifact(fullFile); const full = PNG.sync.read(await fs.readFile(path.join(out, fullFile))); row.pixel_sizes.full_png = [full.width, full.height];
      // Recheck only final assertions after the last declared mutation. Earlier
      // transient/intermediate assertions retain their original ordered receipt.
      if (failedIndex === null) {
        let finalStart = recipe.length; while (finalStart > 0 && assertionActions.has(recipe[finalStart - 1].action)) finalStart--;
        row.post_capture_consistency = [];
        for (let index = finalStart; index < recipe.length; index++) { const step = recipe[index]; try { row.post_capture_consistency.push({ ...step, step_index: index, status: 'passed', actual: await executeAssertion(page, step) }); } catch (error) { row.post_capture_consistency.push({ ...step, step_index: index, status: 'failed', actual: null, error: error.message }); row.errors.push('Final state changed during capture: ' + error.message); } }
      }
      row.artifacts.geometry = await writeJson(`states/${sid}/geometry.json`, { ...await readGeometry(page, selector), before_capture: beforeGeometry, screenshot_method: 'actual first DOM element screenshot plus full viewport; no DOM/CSS patch, mask or resize', observations_before: beforeCapture, observations_after: await observations(page) });
      row.source_dom = await writeSourceDom(page, sid, selector);
      row.inputs = await writeJson(`states/${sid}/inputs.json`, { request_descriptor: input.item, descriptor: input.descriptor, clock_iso: request.clock_iso, device_scale_factor: 1, original_source_identity: report.sources_before });
    } catch (error) { row.errors.push(error.message); row.execution_error = error.stack; }
    finally {
      if (page && !row.artifacts.geometry) { try { row.artifacts.geometry = await writeJson(`states/${sid}/geometry.json`, { ...await readGeometry(page, input.descriptor.target.selector), capture_failed: true }); } catch {} }
      if (page && !row.source_dom) { try { row.source_dom = await writeSourceDom(page, sid, input.descriptor.target.selector); } catch {} }
      if (page && !row.artifacts.full_png) { try { const file = `states/${sid}/full.png`; await page.screenshot({ path: path.join(out, file), animations: 'disabled', fullPage: false }); row.artifacts.full_png = await artifact(file); const png = PNG.sync.read(await fs.readFile(path.join(out, file))); row.pixel_sizes.full_png = [png.width, png.height]; } catch {} }
      row.errors.push(...localErrors.map(item => 'Page error: ' + item.message), ...localConsole.map(item => 'Console error: ' + item.text), ...localExternal.map(item => 'External request rejected: ' + item.url), ...localRequests.map(item => 'Failed source request: ' + item.url));
      row.artifacts.assertions = await writeJson(`states/${sid}/assertions.json`, transcript); row.assertion_count = transcript.length;
      row.status = row.errors.length === 0 && transcript.length > 0 && transcript.every(item => item.status === 'passed') && row.skipped_assertions === 0 && Object.values(row.artifacts).every(Boolean) ? 'passed' : 'failed';
      await context.close(); report.results.push(row);
      console.log(JSON.stringify({ state_id: sid, status: row.status, assertions: row.assertion_count, errors: row.errors }));
    }
  }
  report.sources_after = await sourceIdentities(expectedSources);
  report.source_drift = Object.keys(expectedSources).filter(file => report.sources_after[file] !== report.sources_before[file]);
  if (sha(await fs.readFile(requestPath)) !== report.request.sha256) report.source_drift.push('request.json');
  if (sha(await fs.readFile(runner)) !== report.generator.sha256) report.source_drift.push(runnerRelative);
  report.collection_complete = report.results.length === requests.length && report.fatal_errors.length === 0;
  report.status = report.collection_complete && !report.source_drift.length && report.results.every(row => row.status === 'passed') ? 'passed' : 'failed';
} catch (error) { report.fatal_errors.push({ message: error.message, stack: error.stack }); }
finally {
  try { await browser?.close(); } finally { if (server) await new Promise(resolve => server.close(resolve)); }
  report.finished_at = new Date().toISOString();
  await writeJson('served-files.json', served);
  await writeJson('verification.json', report);
  console.log(JSON.stringify({ status: report.status, collection_complete: report.collection_complete, states: report.results.length, passed: report.results.filter(row => row.status === 'passed').length, fatal_errors: report.fatal_errors, report: path.join(out, 'verification.json') }));
}
process.exitCode = report.status === 'passed' ? 0 : 1;
