/** Real Vite + production React checks. HMR edits only an isolated source copy. */
import test, { before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { launchLockedBrowser, verifyBrowserIdentity } from '../visual/locked-browser.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const output = path.join(root, '.build/live-ui-preview/verification-' + Date.now());
const copy = path.join(output, 'workspace');
const desktop = path.join(copy, 'apps/desktop');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const results = [];
const productionInputs = [
  'scripts/report/general_visual_assets.mjs',
  'apps/desktop/general-visual.html',
  'apps/desktop/tests/native/general/entry.tsx',
  'apps/desktop/tests/native/general/general-port.ts',
  'apps/desktop/tests/native/general/native-context.ts',
  'apps/desktop/tests/native/general/state-contract.ts',
  'apps/desktop/tests/native/general/reference-replay.js',
  'apps/desktop/tests/native/general/fixture-data.json',
  'docs/development/design-approval-general-states-20261108.json',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/promotion.json',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/settings.html',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/state-gap.css',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/state-gap.js',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/assets/icons.js',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/assets/material-tokens.css',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/source/settings.css',
  'docs/design/first-product-v1/candidates/general-state-evidence-20261108/source/settings.js',
];
const reviewMarkers = ['luna-live-ui-preview-v1', 'controlled-memory-only', 'ui-fixture-session', 'luna-review-navigate'];
const hostMarkers = ['luna-live-ui-preview-v1', 'luna-review-navigate'];
let browser, browserIdentity, server, origin, serverOutput = '', ownedReview = false;
let originalReviewPid, productionBundle;
async function command(argv, cwd = root) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    child.stdout.on('data', value => { stdout += value; }); child.stderr.on('data', value => { stderr += value; });
    child.once('error', reject); child.once('exit', code => resolve({ command: argv, cwd, exitCode: code, stdout, stderr }));
  });
}
async function record(name, value) { await fs.writeFile(path.join(output, name + '.json'), JSON.stringify(value, null, 2) + '\n'); }
function localOutputPath(value) {
  assert.equal(typeof value, 'string', 'Production paths must be strings');
  assert.ok(value && value !== '.' && value !== '..' && !/[\\\u0000:?#]/.test(value) && !value.startsWith('/') && path.posix.normalize(value) === value && !value.startsWith('../'), 'Foreign or escaping production path: ' + value);
  return value;
}
function productionReachability(manifest, chunks) {
  const entries = Object.entries(manifest);
  const entryKey = source => {
    const matching = entries.filter(([, value]) => value.isEntry === true && value.src === source);
    assert.equal(matching.length, 1, 'Missing or ambiguous production entry: ' + source);
    return matching[0][0];
  };
  for (const [key, entry] of entries) {
    localOutputPath(key); localOutputPath(entry.file);
    for (const file of [...(entry.css ?? []), ...(entry.assets ?? [])]) localOutputPath(file);
    for (const kind of ['imports', 'dynamicImports']) {
      assert.ok(entry[kind] === undefined || Array.isArray(entry[kind]), 'Malformed production ' + kind + ': ' + key);
      for (const edge of entry[kind] ?? []) { localOutputPath(edge); assert.ok(Object.hasOwn(manifest, edge), 'Missing production edge: ' + edge); }
    }
    const chunk = chunks[entry.file];
    if (entry.file.endsWith('.js')) {
      assert.ok(chunk && Array.isArray(chunk.modules), 'Missing actual production module graph: ' + entry.file);
      for (const kind of ['imports', 'dynamicImports']) {
        const expected = (entry[kind] ?? []).map(edge => manifest[edge].file).sort();
        assert.deepEqual([...chunk[kind]].sort(), expected, 'Manifest does not match actual chunk ' + kind + ': ' + entry.file);
      }
    }
  }
  const main = entryKey('index.html');
  const privateEntries = [entryKey('general-visual.html'), entryKey('tests/native/general/reference-replay.js')];
  const reached = new Set();
  const visit = key => { if (reached.has(key)) return; reached.add(key); const entry = manifest[key]; for (const edge of [...(entry.imports ?? []), ...(entry.dynamicImports ?? [])]) visit(edge); };
  visit(main);
  for (const key of privateEntries) assert.equal(reached.has(key), false, 'Private QA entry reachable from normal main: ' + key);
  for (const key of reached) for (const module of chunks[manifest[key].file]?.modules ?? []) {
    assert.equal(/(?:^|\/)tests\/(?:support|live-preview|native\/general)(?:\/|$)/.test(module.replaceAll('\\', '/')), false, 'Controlled test module reachable from normal main: ' + module);
  }
  return { main, privateEntries, reached: [...reached], files: [...new Set(['index.html', ...[...reached].flatMap(key => { const entry = manifest[key]; return [entry.file, ...(entry.css ?? []), ...(entry.assets ?? [])]; })])] };
}
async function ensureProductionBundle() {
  productionBundle ??= (async () => {
    // Keep the real production config and all its entry points. This extra plugin only
    // observes Rollup's final module graph; it does not rewrite inputs or output.
    const runner = path.join(output, 'build-production-with-graph.mjs');
    await fs.writeFile(runner, `import fs from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
const [vitePath, configFile, graphFile] = process.argv.slice(2);
const { build } = await import(pathToFileURL(vitePath).href);
await build({ configFile, build: { manifest: true }, plugins: [{ name: 'observe-live-preview-production-isolation', async writeBundle(options, bundle) {
  const chunks = Object.fromEntries(Object.entries(bundle).filter(([, item]) => item.type === 'chunk').map(([name, item]) => [name, { modules: Object.keys(item.modules).sort(), imports: item.imports, dynamicImports: item.dynamicImports, facadeModuleId: item.facadeModuleId, sha256: createHash('sha256').update(item.code).digest('hex') }]));
  await fs.writeFile(graphFile, JSON.stringify({ outputDirectory: options.dir, chunks }, null, 2) + '\\n');
} }] });
`);
    const graphPath = path.join(output, 'production-module-graph.json');
    const built = await command([process.execPath, runner, path.join(root, 'apps/desktop/node_modules/vite/dist/node/index.js'), path.join(desktop, 'vite.config.ts'), graphPath], desktop);
    await record('production-bundle', { ...built, runnerSha256: hash(await fs.readFile(runner)), configSha256: hash(await fs.readFile(path.join(desktop, 'vite.config.ts'))), buildManifest: true });
    assert.equal(built.exitCode, 0, 'Actual isolated production build failed; see production-bundle.json: ' + built.stderr);
    const dist = path.join(desktop, 'dist');
    const files = [];
    const readTree = async relative => { for (const item of await fs.readdir(path.join(dist, relative), { withFileTypes: true })) {
      const file = localOutputPath(path.posix.join(relative, item.name));
      assert.equal(item.isSymbolicLink(), false, 'Production output cannot escape through a symlink: ' + file);
      if (item.isDirectory()) await readTree(file); else { assert.ok(item.isFile(), 'Unexpected production output: ' + file); files.push({ file, bytes: await fs.readFile(path.join(dist, file)) }); }
    } };
    await readTree('');
    assert.ok(files.some(item => item.file === 'index.html'), 'Missing actual production index');
    assert.ok(files.some(item => item.file.startsWith('assets/')), 'Missing actual production assets');
    const manifest = JSON.parse(await fs.readFile(path.join(dist, '.vite/manifest.json'), 'utf8'));
    const graph = JSON.parse(await fs.readFile(graphPath, 'utf8'));
    const filesByName = new Map(files.map(item => [item.file, item.bytes]));
    for (const [file, chunk] of Object.entries(graph.chunks)) { localOutputPath(file); assert.ok(filesByName.has(file), 'Missing actual production chunk: ' + file); assert.equal(hash(filesByName.get(file)), chunk.sha256, 'Production graph and actual bytes differ: ' + file); }
    for (const entry of Object.values(manifest)) for (const file of [entry.file, ...(entry.css ?? []), ...(entry.assets ?? [])]) { localOutputPath(file); assert.ok(filesByName.has(file), 'Missing actual manifest output: ' + file); }
    return { built, manifest, chunks: graph.chunks, files, filesByName };
  })();
  return await productionBundle;
}
async function health() { try { return await (await fetch('http://127.0.0.1:1421/__luna_live_preview_health', { signal: AbortSignal.timeout(500) })).json(); } catch { return null; } }
async function open(query = '') {
  const page = await browser.newPage({ viewport: { width: 1380, height: 1020 } });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(origin + '/' + query);
  const frame = await page.locator('#review-client').elementHandle().then(element => element.contentFrame());
  await frame.waitForSelector('#root > *');
  return { page, frame, errors };
}
const check = (name, run) => test(name, { timeout: 60000 }, async () => {
  try { const value = await run(); results.push({ name, status: 'passed', value }); }
  catch (error) { results.push({ name, status: 'failed', error: error.stack }); throw error; }
});

before(async () => {
  await fs.mkdir(output, { recursive: true });
  for (const relative of ['apps/desktop/src', 'apps/desktop/tests/support', 'apps/desktop/tests/live-preview', 'apps/desktop/index.html', 'apps/desktop/vite.config.ts', 'apps/desktop/tsconfig.json', 'apps/desktop/package.json', 'docs/design/first-product-v1/assets']) {
    const destination = path.join(copy, relative); await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.cp(path.join(root, relative), destination, { recursive: true });
  }
  const inputHashes = [];
  for (const relative of productionInputs) {
    const source = path.join(root, relative), destination = path.join(copy, relative);
    await fs.mkdir(path.dirname(destination), { recursive: true }); await fs.copyFile(source, destination);
    const sourceSha256 = hash(await fs.readFile(source)), copySha256 = hash(await fs.readFile(destination));
    assert.equal(copySha256, sourceSha256, 'Production input changed while copying: ' + relative);
    inputHashes.push({ path: relative, sourceSha256, copySha256 });
  }
  await record('isolated-production-input-closure', { inputCount: inputHashes.length, inputs: inputHashes });
  await fs.symlink(path.join(root, 'apps/desktop/node_modules'), path.join(desktop, 'node_modules'), 'dir');
  await fs.symlink(path.join(root, 'node_modules'), path.join(copy, 'node_modules'), 'dir');
  const reservation = createServer(); await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve)); origin = 'http://127.0.0.1:' + port;
  server = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', path.join(desktop, 'tests/live-preview/vite.config.ts'), '--port', String(port), '--strictPort'], { cwd: desktop, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', value => { serverOutput += value; }); server.stderr.on('data', value => { serverOutput += value; });
  for (let i = 0; i < 150; i++) {
    try { if ((await fetch(origin)).ok) break; } catch {}
    if (i === 149 || server.exitCode !== null) throw Error('Isolated Vite failed: ' + serverOutput);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  ({ browser, identity: browserIdentity } = await launchLockedBrowser());
  const existing = await health();
  if (!existing) { const launch = await command([process.execPath, 'scripts/dev/live-ui-preview.mjs']); await record('launcher-start', launch); assert.equal(launch.exitCode, 0); ownedReview = true; }
  const current = await health(); assert.equal(current.repository, root); originalReviewPid = current.pid;
});

after(async () => {
  if (browser) { await verifyBrowserIdentity(browserIdentity); await browser.close(); }
  if (server?.exitCode === null) { server.kill('SIGTERM'); await new Promise(resolve => server.once('exit', resolve)); }
  if (ownedReview) { const current = await health(); if (current?.pid === originalReviewPid) await command([process.execPath, 'scripts/dev/live-ui-preview.mjs', '--stop']); }
  await record('results', { scope: 'live controlled production-component preview; not native or product acceptance', browser: browserIdentity, results });
  await fs.writeFile(path.join(output, 'isolated-vite.log'), serverOutput);
  console.log('Live preview evidence: ' + output);
});

check('actual iframe sizes, root origin and production navigation', async () => {
  const { page, errors } = await open(); const facts = [];
  for (const [view, role, width, height] of [['session', 'main', 1040, 720], ['library', 'main', 1040, 720], ['history', 'main', 1040, 720], ['settings-general', 'settings', 1000, 720], ['settings-translation', 'settings', 1000, 720], ['hud', 'hud', 560, 290], ['learning', 'learning', 1040, 760]]) {
    await page.selectOption('#review-page', view);
    const locator = page.frameLocator('#review-client'); await locator.locator('#root > *').waitFor();
    if (role === 'main') await locator.locator('nav [aria-current="page"]').filter({ hasText: { session: '当前会话', library: '游戏库', history: '历史记录' }[view] }).waitFor();
    const actual = await locator.locator('html').evaluate(element => ({ role: element.dataset.windowRole, width: innerWidth, height: innerHeight, box: document.getElementById('root').getBoundingClientRect().toJSON() }));
    assert.equal(actual.role, role); assert.equal(actual.width, width); assert.equal(actual.height, height); assert.equal(actual.box.x, 0); assert.equal(actual.box.y, 0); assert.equal(actual.box.width, width); assert.equal(actual.box.height, height);
    facts.push({ view, actual }); await page.screenshot({ path: path.join(output, view + '.png') });
  }
  assert.deepEqual(errors, []); await page.close(); return facts;
});

check('page, narrow viewport and theme controls stay outside actual client', async () => {
  const { page, errors } = await open();
  const frame = page.frameLocator('#review-client'); await frame.locator('nav').waitFor();
  await frame.getByRole('button', { name: '历史记录', exact: true }).click(); await page.waitForFunction(() => document.getElementById('review-page').value === 'history');
  await page.selectOption('#review-theme', 'light'); await page.selectOption('#review-size-control', 'narrow');
  await frame.locator('html[data-theme="light"]').waitFor();
  const facts = await frame.locator('html').evaluate(() => ({ width: innerWidth, height: innerHeight, theme: document.documentElement.dataset.theme, toolbarInside: Boolean(document.getElementById('review-page')) }));
  assert.deepEqual(facts, { width: 760, height: 560, theme: 'light', toolbarInside: false });
  assert.match(page.url(), /view=history/); assert.match(page.url(), /size=narrow/); assert.match(page.url(), /theme=light/);
  await page.screenshot({ path: path.join(output, 'history-narrow-light.png') }); assert.deepEqual(errors, []); await page.close(); return facts;
});

check('real production CSS HMR changes a live client without document navigation', async () => {
  const { page, frame, errors } = await open(); await frame.waitForSelector('nav [aria-current="page"]');
  const css = path.join(desktop, 'src/features/reading/styles.module.css'); const before = await fs.readFile(css); const nonce = 'proof' + Date.now();
  await frame.evaluate(value => { window.__liveReviewNavigationProof = value; }, nonce);
  await page.screenshot({ path: path.join(output, 'hmr-before.png') });
  try {
    await fs.appendFile(css, '\n:global(html){--luna-live-review-hmr-proof:' + nonce + ';}\n');
    await frame.waitForFunction(value => getComputedStyle(document.documentElement).getPropertyValue('--luna-live-review-hmr-proof').trim() === value, nonce);
    assert.equal(await frame.evaluate(() => window.__liveReviewNavigationProof), nonce);
    await page.getByText(/^热更新 ·/).waitFor(); await page.screenshot({ path: path.join(output, 'hmr-after.png') });
  } finally { await fs.writeFile(css, before); }
  assert.equal(hash(await fs.readFile(css)), hash(before)); assert.deepEqual(errors, []); await page.close();
  return { editedOnlyIsolatedSource: path.relative(root, css), originalRestoredSha256: hash(before), navigationPreserved: true };
});

check('preview has no native runtime and external fetch is blocked by actual CSP', async () => {
  const { page, frame, errors } = await open(); await frame.waitForSelector('nav');
  const facts = await frame.evaluate(async () => {
    let blockedUri = '';
    document.addEventListener('securitypolicyviolation', event => { blockedUri = event.blockedURI; }, { once: true });
    let rejected = false; try { await fetch('https://example.invalid/luna-preview-negative'); } catch { rejected = true; }
    await new Promise(resolve => setTimeout(resolve, 80));
    return { native: '__TAURI_INTERNALS__' in window, kind: window.liveUiPreview?.kind, rejected, blockedUri };
  });
  assert.equal(facts.native, false); assert.equal(facts.kind, 'controlled-memory-only'); assert.equal(facts.rejected, true); assert.match(facts.blockedUri, /^https:\/\/example\.invalid/);
  const pagesBefore = page.context().pages().length;
  await frame.evaluate(() => { const anchor = document.createElement('a'); anchor.href = 'https://example.invalid/luna-preview-link-negative'; anchor.target = '_blank'; document.body.append(anchor); anchor.click(); anchor.remove(); });
  await page.getByText('预览不会打开外部网址', { exact: true }).waitFor(); assert.equal(page.context().pages().length, pagesBefore);
  const previous = page.url(); await page.evaluate(() => window.postMessage({ kind: 'luna-review-navigate', view: 'hud' }, location.origin)); await page.waitForTimeout(80); assert.equal(page.url(), previous);
  assert.deepEqual(errors, []); await page.close(); return facts;
});

check('empty service selection and disconnected window are rendered honestly', async () => {
  const { page, frame, errors } = await open('?state=disconnected');
  const locator = page.frameLocator('#review-client'); await locator.locator('nav [aria-current="page"]').filter({ hasText: '当前会话' }).waitFor();
  await locator.locator('[data-session-layout="stopped"]').waitFor();
  assert.equal(await locator.locator('[class*="readingGrid"] > aside').count(), 0, 'Stopped unbound session must collapse the unused preview');
  assert.equal(await locator.getByText('尚无可核验的关联窗口', { exact: true }).count(), 0);
  await locator.locator('[data-guided-current] [data-testid="source-text"]').waitFor();
  assert.equal(await locator.getByRole('button', { name: '重新连接游戏…', exact: true }).count(), 1);
  assert.equal(await locator.getByText(/Controlled saved-service projection/).count(), 0);
  assert.equal(await locator.getByText('正在连接采集器', { exact: true }).count(), 0);
  const proof = await frame.evaluate(() => ({ state: window.liveUiPreview.port.state.session.state, binding: window.liveUiPreview.port.bindingFixture.state, leases: window.liveUiPreview.port.previewProjections.size }));
  assert.deepEqual(proof, { state: 'stopped', binding: 'unbound', leases: 0 });
  await locator.getByRole('button', { name: '翻译服务：尚未选择', exact: true }).click();
  await locator.getByRole('dialog').getByText(/还没有已保存的服务/).waitFor();
  assert.equal(await locator.locator('[data-saved-service]').count(), 0);
  await page.screenshot({ path: path.join(output, 'disconnected-empty-service.png') });
  assert.deepEqual(errors, []); await page.close();
  const active = await open('?state=disconnected');
  await active.frame.evaluate(() => { const port = window.liveUiPreview.port; port.publish({ session: { ...port.state.session, state: 'active' } }); });
  const live = active.page.frameLocator('#review-client');
  await live.locator('[data-session-layout="live"]').waitFor();
  await live.getByText('尚无可核验的关联窗口', { exact: true }).waitFor();
  const activeProof = await active.frame.evaluate(() => ({ state: window.liveUiPreview.port.state.session.state, binding: window.liveUiPreview.port.bindingFixture.state, leases: window.liveUiPreview.port.previewProjections.size }));
  assert.deepEqual(activeProof, { state: 'active', binding: 'unbound', leases: 0 });
  assert.equal(await live.getByText(/Controlled saved-service projection/).count(), 0);
  assert.equal(await live.getByText('正在连接采集器', { exact: true }).count(), 0);
  await active.page.screenshot({ path: path.join(output, 'active-unbound-preview.png') });
  assert.deepEqual(active.errors, []); await active.page.close(); return { stoppedUnbound: proof, activeUnbound: activeProof };
});

check('launcher is idempotent and refuses another workspace without stopping the live service', async () => {
  const same = await command([process.execPath, 'scripts/dev/live-ui-preview.mjs']); assert.equal(same.exitCode, 0); assert.match(same.stdout, /已在运行/);
  const foreign = path.join(copy, 'scripts/dev/live-ui-preview.mjs'); await fs.mkdir(path.dirname(foreign), { recursive: true }); await fs.copyFile(path.join(root, 'scripts/dev/live-ui-preview.mjs'), foreign);
  const rejected = await command([process.execPath, foreign], copy); assert.notEqual(rejected.exitCode, 0); assert.match(rejected.stderr, /其他服务或工作区/);
  const current = await health(); assert.equal(current.pid, originalReviewPid); await record('launcher-idempotent', same); await record('launcher-foreign-workspace-negative', rejected);
  return { samePid: current.pid, foreignExitCode: rejected.exitCode, userPreviewStillRunning: true };
});

check('actual production bundle does not include the review host or controlled transport', async () => {
  const bundle = await ensureProductionBundle();
  const reachability = productionReachability(bundle.manifest, bundle.chunks);
  const normalContents = reachability.files.map(file => bundle.filesByName.get(file).toString('utf8')).join('\n');
  for (const marker of reviewMarkers) assert.equal(normalContents.includes(marker), false, 'Development entry leaked into normal main: ' + marker);
  const contents = bundle.files.map(item => item.bytes.toString('utf8')).join('\n');
  for (const marker of hostMarkers) assert.equal(contents.includes(marker), false, 'Live preview host leaked anywhere in production: ' + marker);
  // Exercise failure paths without changing any actual manifest, graph or build output.
  const missing = structuredClone(bundle.manifest); missing[reachability.main].imports = [...(missing[reachability.main].imports ?? []), 'missing-production-edge'];
  assert.throws(() => productionReachability(missing, bundle.chunks), /Missing production edge/);
  const foreign = structuredClone(bundle.manifest); foreign[reachability.main].dynamicImports = ['https://example.invalid/foreign.js'];
  assert.throws(() => productionReachability(foreign, bundle.chunks), /Foreign or escaping production path/);
  const escaping = structuredClone(bundle.manifest); escaping[reachability.main].file = '../outside.js';
  assert.throws(() => productionReachability(escaping, bundle.chunks), /Foreign or escaping production path/);
  const noMain = structuredClone(bundle.manifest); delete noMain[reachability.main];
  assert.throws(() => productionReachability(noMain, bundle.chunks), /Missing or ambiguous production entry/);
  const mainFile = bundle.manifest[reachability.main].file;
  for (const privateKey of reachability.privateEntries) {
    const privateReachable = structuredClone(bundle.manifest), linkedChunks = structuredClone(bundle.chunks);
    privateReachable[reachability.main].dynamicImports = [...(privateReachable[reachability.main].dynamicImports ?? []), privateKey];
    linkedChunks[mainFile].dynamicImports = [...linkedChunks[mainFile].dynamicImports, privateReachable[privateKey].file];
    assert.throws(() => productionReachability(privateReachable, linkedChunks), /Private QA entry reachable from normal main/);
  }
  const forbiddenModules = ['tests/support/design-port.ts', 'tests/live-preview/client.tsx', 'tests/native/general/entry.tsx'];
  for (const relative of forbiddenModules) {
    const moduleLeak = structuredClone(bundle.chunks); moduleLeak[mainFile].modules.push(path.join(desktop, relative));
    assert.throws(() => productionReachability(bundle.manifest, moduleLeak), /Controlled test module reachable from normal main/);
  }
  const audit = { buildExitCode: bundle.built.exitCode, normalEntry: reachability.main, normalReachableEntries: reachability.reached, normalReachableFiles: reachability.files, privateEntriesUnreachable: reachability.privateEntries, normalMarkersAbsent: reviewMarkers, entireBundleHostMarkersAbsent: hostMarkers, productionOutputFiles: bundle.files.map(item => ({ file: item.file, sha256: hash(item.bytes) })), rejectedNegatives: ['missing edge', 'foreign dynamic edge', 'escaping file', 'missing main', ...reachability.privateEntries.map(key => 'reachable private entry: ' + key), ...forbiddenModules.map(module => 'reachable controlled module: ' + module)] };
  await record('production-reachability-audit', audit); return audit;
});

check('real external middle-click cannot open a popup while local links keep their navigation', async () => {
  const { page, frame, errors } = await open(); await frame.waitForSelector('nav');
  const attempted = [];
  await page.context().route('https://example.invalid/**', route => { attempted.push(route.request().url()); return route.abort(); });
  await frame.evaluate(() => {
    for (const [id, href, top] of [['review-external-aux', 'https://example.invalid/live-review-middle-negative', 200], ['review-local-aux', '#live-review-local-middle', 240]]) {
      const anchor = document.createElement('a'); anchor.id = id; anchor.href = href; anchor.target = '_blank'; anchor.textContent = id;
      Object.assign(anchor.style, { position: 'fixed', left: '400px', top: top + 'px', zIndex: '10000' }); document.body.append(anchor);
    }
  });
  const before = page.context().pages().length;
  await frame.locator('#review-external-aux').click({ button: 'middle' });
  await page.getByText('预览不会打开外部网址', { exact: true }).waitFor();
  await page.waitForTimeout(350);
  assert.equal(page.context().pages().length, before); assert.deepEqual(attempted, []);
  await page.screenshot({ path: path.join(output, 'auxclick-no-external-popup.png') });
  const localPopup = page.context().waitForEvent('page');
  await frame.locator('#review-local-aux').click({ button: 'middle' });
  const popup = await localPopup; await popup.waitForLoadState('domcontentloaded');
  assert.equal(new URL(popup.url()).origin, origin); assert.equal(new URL(popup.url()).hash, '#live-review-local-middle');
  await popup.close();
  await frame.locator('#review-local-aux').evaluate(anchor => anchor.removeAttribute('target'));
  await frame.locator('#review-local-aux').click();
  assert.equal(await frame.evaluate(() => location.hash), '#live-review-local-middle');
  assert.deepEqual(errors, []); await page.close();
  return { externalMiddlePopupBlocked: true, attemptedExternalNavigation: attempted, actualExternalRequestsPerformed: false, localMiddlePopupAllowed: true, localHashNavigationPreserved: true };
});

check('the real client boundary keeps 900 wide and switches at 899 without clipping the initial title', async () => {
  const facts = [];
  for (const [width, height, sidebarWidth, columns] of [[1040, 720, 190, 2], [900, 720, 190, 2], [899, 720, 57, 1], [760, 560, 57, 1]]) {
    const page = await browser.newPage({ viewport: { width, height } }); const errors = []; page.on('pageerror', error => errors.push(error.message));
    await page.goto(origin + '/client.html?role=main&view=session&state=normal');
    await page.getByRole('navigation', { name: '主导航' }).locator('[aria-current="page"]').filter({ hasText: '当前会话' }).waitFor();
    await page.evaluate(() => {
      const port = window.liveUiPreview.port, gameId = 'live-review-long-title';
      const game = { gameId, displayName: 'サクラノ詩 10th Anniversary edition', originalTitle: 'サクラノ詩', sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1,
        target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } };
      port.publish({ games: [game], session: { ...port.state.session, gameId, state: 'stopped' } });
    });
    await page.getByRole('heading', { name: 'サクラノ詩 10th Anniversary edition', exact: true }).waitFor();
    await page.locator('[data-session-layout="stopped"] [class*="readingGrid"][data-verified-window="true"] > aside').waitFor();
    const bindingProof = await page.evaluate(() => { const port = window.liveUiPreview.port; return { state: port.state.session.state, binding: port.bindingFixture.state, sessionId: port.state.session.sessionId, bindingSessionId: port.bindingFixture.sessionId }; });
    assert.equal(bindingProof.state, 'stopped'); assert.equal(bindingProof.binding, 'bound'); assert.ok(bindingProof.sessionId); assert.equal(bindingProof.bindingSessionId, bindingProof.sessionId);
    const geometry = await page.getByRole('heading', { name: 'サクラノ詩 10th Anniversary edition', exact: true }).evaluate(title => {
      const owner = [...document.querySelectorAll('*')].filter(element => getComputedStyle(element).containerName.split(' ').includes('main-client'));
      const sidebar = document.querySelector('nav[aria-label="主导航"]').closest('aside');
      const session = title.closest('[data-ui-scroll="main-content"]');
      const grid = session.querySelector('[class*="readingGrid"]'), preview = grid.querySelector(':scope > aside'), current = grid.firstElementChild;
      let clip = { left: 0, top: 0, right: innerWidth, bottom: innerHeight };
      for (let ancestor = title; ancestor; ancestor = ancestor.parentElement) {
        const style = getComputedStyle(ancestor), box = ancestor.getBoundingClientRect();
        if (/(hidden|clip|auto|scroll)/.test(style.overflowX)) { clip.left = Math.max(clip.left, box.left + ancestor.clientLeft); clip.right = Math.min(clip.right, box.left + ancestor.clientLeft + ancestor.clientWidth); }
        if (/(hidden|clip|auto|scroll)/.test(style.overflowY)) { clip.top = Math.max(clip.top, box.top + ancestor.clientTop); clip.bottom = Math.min(clip.bottom, box.top + ancestor.clientTop + ancestor.clientHeight); }
      }
      const range = document.createRange(); range.selectNodeContents(title);
      return { width: innerWidth, owners: owner.map(element => ({ width: element.getBoundingClientRect().width, border: getComputedStyle(element).borderLeftWidth })), sidebar: sidebar.getBoundingClientRect().width,
        grid: grid.getBoundingClientRect().toJSON(), columns: getComputedStyle(grid).gridTemplateColumns.split(/\s+/), preview: preview.getBoundingClientRect().toJSON(), current: current.getBoundingClientRect().toJSON(),
        title: title.getBoundingClientRect().toJSON(), titleText: title.textContent, textRects: [...range.getClientRects()].map(box => box.toJSON()), clip, sessionScrollTop: session.scrollTop,
        document: { htmlTop: document.documentElement.scrollTop, bodyTop: document.body.scrollTop, scrollHeight: document.documentElement.scrollHeight, scrollWidth: document.documentElement.scrollWidth, viewportHeight: innerHeight } };
    });
    assert.equal(geometry.width, width); assert.equal(geometry.owners.length, 1); assert.equal(geometry.owners[0].width, width); assert.equal(geometry.owners[0].border, '0px');
    assert.equal(geometry.sidebar, sidebarWidth); assert.equal(geometry.columns.length, columns); assert.equal(geometry.preview.width, 282);
    if (columns === 2) assert.ok(geometry.preview.left >= geometry.current.right, 'Wide preview must stay beside the reading column');
    else { assert.equal(geometry.preview.left, geometry.grid.left); assert.ok(geometry.preview.top >= geometry.current.bottom, 'Narrow preview follows reading content'); }
    assert.equal(geometry.sessionScrollTop, 0); assert.ok(geometry.textRects.length > 0);
    for (const box of geometry.textRects) {
      assert.ok(box.left >= geometry.clip.left - .5 && box.right <= geometry.clip.right + .5 && box.top >= geometry.clip.top - .5 && box.bottom <= geometry.clip.bottom + .5, 'Every initial title text fragment must remain in its actual ancestor clip');
    }
    assert.equal(geometry.document.htmlTop, 0); assert.equal(geometry.document.bodyTop, 0); assert.ok(geometry.document.scrollHeight <= geometry.document.viewportHeight); assert.ok(geometry.document.scrollWidth <= width);
    assert.deepEqual(errors, []); await page.screenshot({ path: path.join(output, 'boundary-' + width + '.png') }); facts.push({ ...geometry, bindingProof }); await page.close();
  }
  for (const role of ['settings', 'hud']) {
    const page = await browser.newPage({ viewport: { width: 900, height: 720 } });
    await page.goto(origin + '/client.html?role=' + role + '&view=' + (role === 'settings' ? 'settings-general' : 'hud'));
    await page.locator('#root > *').waitFor();
    const owners = await page.evaluate(() => [...document.querySelectorAll('*')].filter(element => getComputedStyle(element).containerName.split(' ').includes('main-client')).length);
    assert.equal(owners, 0, 'Main client container cannot leak into ' + role); await page.close();
  }
  return { scope: 'Browser production-component layout only; not Native acceptance', clientBoundaries: facts, otherWindowRolesWithoutMainOwner: ['settings', 'hud'] };
});

check('toolbar design comparison uses authored reading data across reload without real authority or shipped seed', async () => {
  const text = await fs.readFile(path.join(root, 'docs/design/first-product-v1/data.js'), 'utf8');
  const literal = /window\.LunaDesignData\s*=\s*(\{[\s\S]*?\n\});/.exec(text);
  assert.ok(literal, 'Authored static design data must remain explicitly readable without eval');
  const authored = JSON.parse(literal[1]), game = authored.games.find(item => item.id === 'sakura'), line = game.lines[0];
  const { page, errors } = await open(); const externalRequests = [];
  page.context().on('request', request => { const url = new URL(request.url()); if (['http:', 'https:'].includes(url.protocol) && url.origin !== origin) externalRequests.push(url.href); });
  await page.selectOption('#review-state', 'design-reference');
  const read = async () => {
    const current = page.frameLocator('#review-client');
    await current.getByRole('heading', { name: game.title, exact: true }).waitFor();
    await current.locator('[data-guided-current] > footer').getByText('已完成', { exact: true }).waitFor();
    await current.getByRole('button', { name: '翻译服务：Microsoft · 日中翻译', exact: true }).waitFor();
    return await current.locator('html').evaluate(async () => {
      const review = window.liveUiPreview, port = review.port, unit = port.state.current, article = document.querySelector('[data-guided-current]');
      const copy = article.querySelector('[data-testid="source-text"]').cloneNode(true); copy.querySelectorAll('rt').forEach(node => node.remove());
      const services = await port.savedServices({ expectedProfileGeneration: port.state.profileGeneration, scope: { kind: 'global' }, purpose: 'translation' });
      return { id: review.designSample.id, provenance: review.designSample.provenance, game: port.state.games[0].displayName, source: unit.sourceText, sourceHash: unit.sourceHash,
        displayedSource: copy.textContent, translation: unit.translations[0].text, displayedTranslation: [...article.querySelectorAll('p')].map(node => node.textContent),
        provider: unit.translations[0].providerLabel, tokens: unit.tokens.map(token => [token.surface, token.reading, token.lemma, token.partOfSpeech]),
        preferences: { source: port.state.preferences.sourceFontPt, translation: port.state.preferences.translationFontPt }, native: '__TAURI_INTERNALS__' in window,
        binding: port.bindingFixture.state, previewLeases: port.previewProjections.size, previewFrames: port.previewFrameFixtures.size, previewPermission: port.previewCapabilityFixture.permission,
        serviceEligibility: services.rows[0].eligibility.kind, serviceVerification: services.rows[0].verification.kind, purposeEnabled: services.selection.purposeEnabled };
    });
  };
  const first = await read();
  assert.equal(first.game, game.title); assert.equal(first.source, line.source); assert.equal(first.displayedSource, line.source); assert.equal(first.translation, line.translation); assert.ok(first.displayedTranslation.includes(line.translation));
  assert.equal(first.sourceHash, hash(Buffer.from(line.source))); assert.deepEqual(first.tokens, line.tokens.map(token => [token.surface, token.reading, token.lemma, token.pos]));
  assert.deepEqual(first.preferences, { source: 18, translation: 24 }); assert.equal(first.native, false); assert.equal(first.binding, 'unbound'); assert.equal(first.previewLeases, 0); assert.equal(first.previewFrames, 0); assert.equal(first.previewPermission, 'not_requested');
  assert.equal(first.serviceEligibility, 'blocked'); assert.equal(first.serviceVerification, 'untested'); assert.equal(first.purposeEnabled, false);
  assert.equal(hash(await fs.readFile(path.join(root, first.provenance.source))), first.provenance.sourceSha256);
  assert.equal(hash(Buffer.from(text)), first.provenance.dataSourceSha256);
  await page.locator('#review-design-sample-note').waitFor({ state: 'visible' });
  await page.screenshot({ path: path.join(output, 'design-reference-normal.png') }); await page.locator('#review-client').screenshot({ path: path.join(output, 'design-reference-client.png') });
  assert.match(page.url(), /state=design-reference/); await page.reload(); const reloaded = await read(); assert.deepEqual(reloaded, first);
  assert.equal(await page.locator('#review-state').inputValue(), 'design-reference'); assert.deepEqual(externalRequests, []); assert.deepEqual(errors, []);
  const bundle = await ensureProductionBundle(); const contents = bundle.files.map(item => item.bytes.toString('utf8')).join('\n');
  assert.equal(contents.includes(first.id), false, 'Design comparison seed must not enter the actual production bundle');
  await page.close(); return { authoredDataSource: first.provenance.dataSource, dataSourceSha256: first.provenance.dataSourceSha256, sameTitleSourceTranslationTokens: true, unchangedFontPreferences: first.preferences,
    reloadPreserved: true, externalRequests, native: false, previewFrames: 0, realServiceQualified: false, seedInProductionBundle: false };
});

check('manual translation settings uses the typed port and the existing outer preview route', async () => {
  const { page, frame, errors } = await open('?view=manual&state=empty');
  const external = []; page.context().on('request', request => { const url = new URL(request.url()); if (['http:', 'https:'].includes(url.protocol) && url.origin !== origin) external.push(url.href); });
  await frame.getByRole('heading', { name: '手动翻译', exact: true }).waitFor();
  await page.evaluate(() => {
    window.typedSettingsPreviewMessages = [];
    window.addEventListener('message', event => {
      if (event.origin === location.origin && event.source === document.getElementById('review-client').contentWindow && event.data?.kind === 'luna-review-navigate') window.typedSettingsPreviewMessages.push(event.data);
    });
  });
  await frame.getByRole('button', { name: '翻译设置…', exact: true }).click();
  await page.waitForFunction(() => document.getElementById('review-page').value === 'settings-translation');
  const settings = page.frameLocator('#review-client');
  await settings.locator('nav [aria-current="page"]').filter({ hasText: '翻译服务' }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.typedSettingsPreviewMessages), [{ kind: 'luna-review-navigate', view: 'settings-translation' }]);
  assert.match(page.url(), /view=settings-translation/); assert.deepEqual(errors, []); assert.deepEqual(external, []);
  await page.screenshot({ path: path.join(output, 'manual-typed-settings-preview.png') }); await page.close();
  return { actualPublicClick: true, typedPreviewRoute: 'settings-translation', externalRequests: external, native: false };
});
