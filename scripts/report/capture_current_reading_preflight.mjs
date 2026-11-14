/** CP00-A diagnostic capture. Never a product/reference approval adapter. */
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';

const [rootArgument, outputArgument, contextArgument] = process.argv.slice(2);
assert.ok(rootArgument && outputArgument && contextArgument, 'root, fresh output and context required');
const root = path.resolve(rootArgument), out = path.resolve(outputArgument), contextPath = path.resolve(contextArgument);
assert.ok(out.startsWith(path.join(root, '.build') + path.sep));
assert.equal(contextPath, path.join(out, 'context.json'));
assert.deepEqual((await fs.readdir(out)).sort(), ['context.json'], 'refuse existing output artifacts');
const context = JSON.parse(await fs.readFile(contextPath, 'utf8'));
const expectedVariants = [
  { id: 'dark', viewport: { width: 1040, height: 760 }, theme: 'dark', zoom: 1 },
  { id: 'light', viewport: { width: 1040, height: 760 }, theme: 'light', zoom: 1 },
  { id: 'narrow', viewport: { width: 740, height: 1200 }, theme: 'dark', zoom: 1 },
  { id: 'zoom200', viewport: { width: 1040, height: 760 }, theme: 'dark', zoom: 2 },
];
assert.deepEqual(context.variants, expectedVariants, 'exact fixed diagnostic variants required');
const sha = data => createHash('sha256').update(data).digest('hex');
const require = createRequire(path.join(root, 'package.json'));
const { chromium } = require('playwright-core');
const { PNG } = require('pngjs');
const desktop = path.join(root, 'apps/desktop');
const designEntry = 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html';
const reactEntry = 'apps/desktop/tests/visual/index.html';
const csp = JSON.parse(await fs.readFile(path.join(desktop, 'src-tauri/tauri.conf.json'), 'utf8')).app.security.csp;
assert.equal(typeof csp, 'string');
const inputSandbox = { window: {} };
vm.runInNewContext(await fs.readFile(path.join(root, 'docs/design/first-product-v1/data.js'), 'utf8'), inputSandbox, { timeout: 2000 });
const sample = JSON.parse(JSON.stringify(inputSandbox.window.LunaDesignData.manualSample));
const input = { source_text: sample.source, translation: sample.translation };
assert.ok(input.source_text && input.translation);
let assertions = 0;
const check = (truth, message) => { assert.ok(truth, message); assertions++; };
const pageErrors = [], consoleErrors = [], externalRequests = [], requests = [];
const report = {
  schema_version: 1, kind: 'current-reading-preflight/v1', status: 'capture_failed',
  execution_nonce: context.execution_nonce, source_tree_sha256: context.source_tree_sha256,
  design_closure_sha256: context.design_closure_sha256, source_hashes: context.source_hashes,
  command: context.command, started_at: new Date().toISOString(), finished_at: null,
  browser_version: null, reference_approval: 'pending_independent_review',
  claims_product: false, product_alignment_approved: false, native_verified: false,
  packaged_native_tests_executed: false, controlled_react_executed: false,
  skipped_assertions: 0, assertion_count: 0, page_errors: pageErrors,
  console_errors: consoleErrors, external_requests: externalRequests, variants: [],
  capture_scope: 'full_viewport_diagnostic; desktop/test-host chrome retained; no mask or injected matching style',
  product_css_or_components_copied_to_reference: false,
  production_csp: csp, build_commands: [], source_maps: [], requests,
};
await fs.mkdir(path.join(out, 'raw'));
await fs.mkdir(path.join(out, 'captures'));
await fs.mkdir(path.join(out, 'logs'));
const asset = async relative => {
  const bytes = await fs.readFile(path.join(out, relative));
  return { path: relative, bytes: bytes.length, sha256: sha(bytes) };
};
const writeJson = async (relative, value) => {
  await fs.writeFile(path.join(out, relative), JSON.stringify(value, null, 2) + '\n');
  return asset(relative);
};
async function build(argv, label) {
  const command = [process.execPath, ...argv];
  const child = spawn(command[0], command.slice(1), { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  child.stdout.on('data', chunk => { stdout += chunk; }); child.stderr.on('data', chunk => { stderr += chunk; });
  const code = await new Promise((resolve, reject) => { child.once('error', reject); child.once('close', resolve); });
  const stdoutRef = await writeJson(`logs/${label}.json`, { command, cwd: 'apps/desktop', exit_code: code, stdout, stderr });
  report.build_commands.push({ command, exit_code: code, log: stdoutRef });
  check(code === 0, `${label} failed: ${stderr}`);
}
async function listFiles(directory) {
  const files = [];
  for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...await listFiles(full)); else if (entry.isFile()) files.push(full);
  }
  return files.sort();
}
let browser, server;
try {
  const testBuild = path.join(out, 'test-build');
  await build([path.join(desktop, 'node_modules/typescript/bin/tsc'), '--noEmit', '-p', 'tests/tsconfig.json'], 'typecheck-tests');
  await build([path.join(desktop, 'node_modules/vite/bin/vite.js'), 'build', '--config', path.join(desktop, 'tests/visual/vite.config.ts'), '--outDir', testBuild, '--sourcemap', '--manifest'], 'controlled-production-components-build');
  for (const full of (await listFiles(testBuild)).filter(file => file.endsWith('.js.map'))) {
    const map = JSON.parse(await fs.readFile(full, 'utf8'));
    report.source_maps.push({ ...(await asset(path.relative(out, full))), sources: map.sources });
  }
  const graph = report.source_maps.flatMap(map => map.sources);
  for (const component of ['src/app/WindowRoot.tsx', 'src/features/reading/ReadingPanel.tsx', 'src/bridge/projections/application-store.ts'])
    check(graph.some(source => source.endsWith(component)), `actual built source graph must contain ${component}`);
  const mime = { '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css', '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2' };
  server = http.createServer(async (request, response) => {
    try {
      const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
      const designRequest = pathname.startsWith('/docs/design/first-product-v1/');
      const base = designRequest ? root : testBuild;
      const file = path.resolve(base, '.' + pathname);
      const allowedBase = designRequest ? path.join(root, 'docs/design/first-product-v1') : testBuild;
      if (!file.startsWith(allowedBase + path.sep) || !fsSync.statSync(file).isFile()) throw Error('not a regular allowed asset');
      const bytes = await fs.readFile(file);
      const headers = { 'content-type': mime[path.extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' };
      if (!designRequest && file.endsWith('.html')) headers['content-security-policy'] = csp;
      response.writeHead(200, headers); response.end(bytes);
    } catch { response.writeHead(404); response.end('not found'); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  report.server_origins = { design: origin, react: origin };
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--disable-gpu', '--force-color-profile=srgb'] });
  report.browser_version = browser.version();
  for (const variant of expectedVariants) {
    const pair = {};
    for (const producer of ['design', 'react']) {
      const page = await browser.newPage({ viewport: variant.viewport, deviceScaleFactor: 1, colorScheme: variant.theme, reducedMotion: 'reduce' });
      page.on('pageerror', error => pageErrors.push({ variant: variant.id, producer, message: error.message }));
      page.on('console', message => { if (message.type() === 'error') consoleErrors.push({ variant: variant.id, producer, message: message.text() }); });
      await page.route('**/*', async route => {
        const url = new URL(route.request().url());
        if (url.origin !== origin && !['data:', 'blob:'].includes(url.protocol)) { externalRequests.push({ variant: variant.id, producer, url: url.href }); await route.abort(); return; }
        requests.push({ variant: variant.id, producer, path: url.pathname }); await route.continue();
      });
      if (producer === 'react') {
        await page.addInitScript(({ input, theme }) => {
          window.__initializeControlledPort = port => {
            if (!port.state.current || port.state.current.sourceText !== input.source_text) throw Error('existing fixture source differs from design sample');
            const unit = { ...port.state.current, translations: [{ translationId: 'cp00-reading-translation', taskId: 'cp00-reading-task', text: input.translation, providerLabel: 'CP00 controlled transport', targetLanguage: 'zh-CN', correction: null }] };
            port.state.current = unit; port.state.history.units = [unit];
            port.state.preferences = { ...port.state.preferences, theme };
          };
        }, { input, theme: variant.theme });
        const response = await page.goto(`${origin}/tests/visual/index.html?role=main&scenario=reading`);
        check(await response.headerValue('content-security-policy') === csp, 'real React document must enforce unchanged production CSP');
        await page.getByRole('navigation', { name: '主导航' }).getByRole('button', { name: '当前会话', exact: true }).click();
        await page.getByRole('article', { name: '当前原文与译文', exact: true }).waitFor();
        await page.getByText(input.translation, { exact: true }).first().waitFor();
      } else {
        await page.goto(`${origin}/${designEntry}?scenario=reading&theme=${variant.theme}`);
        await page.waitForFunction(() => !!window.LunaFlatDesign && !!window.LunaDesign?.state.current);
        await page.evaluate(({ sample, theme }) => {
          const app = window.LunaDesign, original = app.state.current;
          app.state.current = { ...original, ...sample, id: original.id, recordId: original.recordId, gameId: original.gameId, sessionId: original.sessionId, revision: original.revision, translationStatus: 'complete', time: '10:30', provider: 'CP00 controlled transport' };
          app.state.history = [app.state.current]; app.state.theme = theme;
          app.state.windows.main = true; app.state.windows.hud = false; app.state.windows.game = false;
          app.state.page = 'session'; app.render();
        }, { sample, theme: variant.theme });
        await page.locator('[data-guided-current="true"]').waitFor();
        await page.getByText(input.translation, { exact: true }).first().waitFor();
      }
      if (variant.zoom !== 1) await page.evaluate(zoom => { document.documentElement.style.zoom = String(zoom); }, variant.zoom);
      await page.evaluate(() => document.fonts.ready);
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const observation = await page.evaluate(({ producer, input, entry }) => {
        const surface = producer === 'design' ? document.querySelector('#main-content') : document.querySelector('article[aria-label="当前原文与译文"]')?.closest('[data-ui-scroll="main-content"]');
        if (!surface) throw Error('actual reading surface missing');
        const card = producer === 'design' ? surface.querySelector('[data-guided-current="true"]') : surface.querySelector('article[aria-label="当前原文与译文"]');
        const plain = element => { if (!element) return ''; const clone = element.cloneNode(true); clone.querySelectorAll('rt').forEach(rt => rt.remove()); return clone.textContent.trim(); };
        const source = producer === 'design' ? card?.querySelector('.source') : card?.querySelector('[lang="ja"]');
        const translation = producer === 'design' ? card?.querySelector('.translation') : [...(card?.querySelectorAll('p') ?? [])].find(node => node.textContent.trim() === input.translation);
        const structureSelectors = { current_card: '[data-guided-current="true"]', body_disclosure: 'details[data-flat-session-section="reading"]', extraction_disclosure: 'details[data-flat-session-section="texts"]', hook_disclosure: 'details[data-flat-session-section="logs"]', mutually_exclusive_tabs: '[role="tablist"]' };
        const raw = producer === 'design' ? window.LunaDesign.state.current : window.uiHarness.state.current;
        const box = element => element ? Object.fromEntries(['x', 'y', 'width', 'height'].map(key => [key, element.getBoundingClientRect()[key]])) : null;
        const rootElement = producer === 'design' ? document.querySelector('#main-window') : document.querySelector('#root');
        return {
          producer: producer === 'design' ? 'design' : 'production_react', url: location.href,
          viewport: { width: innerWidth, height: innerHeight }, theme: document.documentElement.dataset.theme,
          zoom: Number(getComputedStyle(document.documentElement).zoom),
          document_attributes: Object.fromEntries([...document.documentElement.attributes].map(attribute => [attribute.name, attribute.value])),
          body_attributes: Object.fromEntries([...document.body.attributes].map(attribute => [attribute.name, attribute.value])),
          computed_conditions: { theme_attribute: document.documentElement.dataset.theme, body_zoom: Number(getComputedStyle(document.body).zoom), document_zoom: Number(getComputedStyle(document.documentElement).zoom), device_scale_factor: devicePixelRatio, viewport_width: innerWidth, viewport_height: innerHeight, root_bounds: box(rootElement) },
          source_text: producer === 'design' ? raw.source : raw.sourceText,
          translation: producer === 'design' ? raw.translation : raw.translations[0]?.text,
          observed_source_text: plain(source), observed_translation: plain(translation),
          structure: Object.fromEntries(Object.entries(structureSelectors).map(([key, selector]) => [key, !!surface.querySelector(selector)])), structure_selectors: structureSelectors,
          page_text: plain(surface), visible_page_text: surface.innerText, visible_text: surface.innerText, raw_unit: raw, html: surface.outerHTML, root_html: surface.outerHTML,
          observed_visibility: Object.fromEntries([['source', source], ['translation', translation]].map(([key, element]) => { const rect = element?.getBoundingClientRect(); return [key, { rendered: !!rect && rect.width > 0 && rect.height > 0, completely_within_viewport: !!rect && rect.width > 0 && rect.height > 0 && rect.left >= 0 && rect.top >= 0 && rect.right <= innerWidth && rect.bottom <= innerHeight, rect: box(element) }]; })),
          geometry: { viewport_width: innerWidth, viewport_height: innerHeight, scroll_width: document.documentElement.scrollWidth, scroll_height: document.documentElement.scrollHeight, reading_surface: box(surface), current_card: box(card), window: box(rootElement) },
          provenance: { entry, controlled_transport: producer !== 'design', production_styles_modified: false },
        };
      }, { producer, input, entry: producer === 'design' ? designEntry : reactEntry });
      const dom = await writeJson(`raw/${variant.id}-${producer}.dom.json`, observation);
      check(observation.source_text === input.source_text && observation.translation === input.translation, 'both producer transports must have identical input');
      check(observation.observed_source_text === input.source_text, `actual ${producer} DOM must contain the source sentence`);
      check(observation.observed_translation === input.translation, `actual ${producer} DOM must contain the translation`);
      check(observation.theme === variant.theme && observation.zoom === variant.zoom, 'actual theme/zoom must match fixed capture condition');
      const screenshotPath = `captures/${variant.id}-${producer}.png`;
      await page.screenshot({ path: path.join(out, screenshotPath), fullPage: false, animations: 'disabled' });
      const screenshot = { ...(await asset(screenshotPath)), ...variant.viewport };
      pair[producer] = { dom, screenshot };
      await page.close();
    }
    const design = PNG.sync.read(await fs.readFile(path.join(out, pair.design.screenshot.path)));
    const react = PNG.sync.read(await fs.readFile(path.join(out, pair.react.screenshot.path)));
    check(design.width === react.width && design.height === react.height, 'same actual PNG dimensions required');
    const diff = new PNG({ width: design.width, height: design.height }); let differentPixels = 0;
    for (let index = 0; index < design.data.length; index += 4) {
      const unequal = [0, 1, 2, 3].some(channel => design.data[index + channel] !== react.data[index + channel]);
      if (unequal) differentPixels++;
      diff.data.set(unequal ? [255, 0, 0, 255] : [48, 48, 48, 255], index);
    }
    const diffPath = `captures/${variant.id}-diff.png`;
    await fs.writeFile(path.join(out, diffPath), PNG.sync.write(diff));
    const total = design.width * design.height;
    report.variants.push({ ...variant, input, ...pair, comparison: { different_pixels: differentPixels, total_pixels: total, ratio: differentPixels / total, pixel_comparison_eligible: false, scope: 'diagnostic_full_viewport_hosts_differ_not_release_metric', diff: { ...(await asset(diffPath)), width: design.width, height: design.height } }, alignment_status: differentPixels ? 'failed' : 'blocked' });
  }
  check(pageErrors.length === 0 && consoleErrors.length === 0, 'browser runtime errors are not acceptable capture evidence');
  check(externalRequests.length === 0, 'no external service may be invoked by diagnostic capture');
  report.status = 'capture_complete'; report.controlled_react_executed = true;
} catch (error) {
  report.error = { name: error.name, message: error.message, stack: error.stack };
  process.exitCode = 1;
} finally {
  if (browser) await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  report.source_hashes_after = {};
  for (const relative of Object.keys(context.source_hashes ?? {}).sort()) report.source_hashes_after[relative] = sha(await fs.readFile(path.join(root, relative)));
  if (JSON.stringify(report.source_hashes_after) !== JSON.stringify(Object.fromEntries(Object.entries(context.source_hashes ?? {}).sort()))) { report.status = 'capture_failed'; report.source_drift = true; process.exitCode = 1; }
  report.assertion_count = assertions; report.finished_at = new Date().toISOString();
  await fs.writeFile(path.join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ status: report.status, variants: report.variants.length, assertion_count: assertions, alignment_status: 'not_approved_by_diagnostic', output: out }));
}
