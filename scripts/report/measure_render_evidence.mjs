#!/usr/bin/env node
/** Independent retained-DOM remeasurement. Never starts the product. */
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import os from 'node:os';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const argv = process.argv.slice(2);
const arg = key => argv[argv.indexOf(key) + 1];
if (argv.length !== 6 || !argv.includes('--root') || !argv.includes('--request') || !argv.includes('--output')) throw Error('Exact --root --request --output required');
const root = await fs.realpath(arg('--root'));
const rawOutput = arg('--output');
const requestPath = path.resolve(arg('--request')), out = path.resolve(rawOutput);
const self = fileURLToPath(import.meta.url), sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hashFile = async file => sha(await fs.readFile(file));
const require = createRequire(path.join(root, 'package.json'));
const { chromium } = require('playwright-core');
const started = new Date().toISOString();
let browser, outputCreated = false;
const report = { schema_version: 1, kind: 'offline-render-measurement/v1', status: 'failed', started_at: started,
  finished_at: null, nonce: null, command: [process.execPath, self, ...argv], cwd: process.cwd(),
  measurement_source: 'independent_browser_on_retained_dom', javascript_enabled: false,
  network_policy: 'offline_allowlisted_bytes_only', scope: 'offline_dom_remeasurement_only',
  native_verified: false, product_alignment_approved: false, reference_approved: false, completion_allowed: false,
  subjects: [], gaps: [], errors: [], assets: [],
  environment: { node: process.version, platform: process.platform, architecture: process.arch, os_release: os.release(),
    playwright: require('playwright-core/package.json').version, locale: 'zh-CN', timezone: 'Asia/Shanghai' },
};
function need(ok, why) { if (!ok) throw Error(why); }
function finiteInt(v, low, high) { return typeof v === 'number' && Number.isInteger(v) && v >= low && v <= high; }
function tree() {
  const code = 'import sys,json,hashlib;from pathlib import Path;r=Path(sys.argv[1]);sys.path.insert(0,str(r/"scripts/quality"));from gate_core import source_files;d=source_files(r);print(json.dumps({"sha256":hashlib.sha256(json.dumps(d,sort_keys=True).encode()).hexdigest(),"file_count":len(d)}))';
  const r = spawnSync('python3', ['-c', code, root], { cwd: root, encoding: 'utf8', timeout: 30000 });
  need(r.status === 0, 'source inventory failed: ' + r.stderr); return JSON.parse(r.stdout);
}
async function pinned(ref) {
  need(ref && typeof ref.path === 'string' && /^[a-f0-9]{64}$/.test(ref.sha256) && finiteInt(ref.bytes, 1, 64 * 1024 * 1024), 'exact positive file descriptor required');
  need(!path.isAbsolute(ref.path) && !ref.path.split('/').some(p => !p || p === '.' || p === '..' || p.includes('\\')), 'safe repository-relative file required');
  let current = root;
  for (const part of ref.path.split('/')) { current = path.join(current, part); need(!(await fs.lstat(current)).isSymbolicLink(), 'symlink evidence forbidden'); }
  need((await fs.stat(current)).isFile(), 'regular evidence file required');
  const bytes = await fs.readFile(current); need(bytes.length === ref.bytes && sha(bytes) === ref.sha256, 'immutable resource hash/size differs: ' + ref.path);
  return { file: current, bytes };
}
function validateSubject(s) {
  need(s && /^[a-zA-Z0-9_-]{1,60}$/.test(s.id), 'safe unique subject ID required');
  need(['guard_fixture', 'immutable_snapshot'].includes(s.evidence_kind), 'explicit evidence kind required');
  const u = new URL(s.document_url); need(['http:', 'https:'].includes(u.protocol) && !u.username && !u.password, 'original HTTP(S) document URL required');
  need(s.viewport && finiteInt(s.viewport.width, 200, 4096) && finiteInt(s.viewport.height, 200, 4096)
    && typeof s.viewport.device_scale_factor === 'number' && [1, 2].includes(s.viewport.device_scale_factor), 'finite CSS px viewport and fixed DPR required');
  need(s.document && ['html', 'dom_json'].includes(s.document.format), 'retained full DOM document required');
  need(s.regions && ['window', 'layout', 'source', 'translation'].every(k => typeof s.regions[k] === 'string' && s.regions[k].length > 0 && s.regions[k].length <= 1024), 'exact region selectors required');
  need(Array.isArray(s.assets) && s.assets.length <= 200, 'bounded explicit asset manifest required');
  need(Array.isArray(s.media_queries) && s.media_queries.every(x => typeof x === 'string' && x.length <= 300), 'media query observations required');
  need(['target', 'whole_client'].includes(s.required_scope), 'explicit requested scope required');
  need(s.required_properties && typeof s.required_properties.focus === 'boolean' && typeof s.required_properties.form_properties === 'boolean', 'typed reconstruction requirements required');
  need(typeof s.token_selector === 'string' && typeof s.pos_selector === 'string', 'annotation selectors required');
}
async function measure(s) {
  validateSubject(s);
  const loaded = await pinned(s.document.file);
  const raw = loaded.bytes.toString('utf8');
  const html = s.document.format === 'html' ? raw : JSON.parse(raw).document_html;
  need(typeof html === 'string' && /<html[\s>]/i.test(html) && /<body[\s>]/i.test(html), 'complete retained document_html required; fragments cannot be promoted');
  const resources = new Map(), refs = [];
  let localFontRules = /local\s*\(/i.test(html);
  for (const a of s.assets) {
    const url = new URL(a.url).href;
    need(['http:', 'https:'].includes(new URL(url).protocol) && !resources.has(url), 'unique absolute offline asset URL required');
    need(['text/css', 'font/woff2', 'font/woff', 'font/ttf', 'font/otf', 'image/png', 'image/jpeg', 'image/svg+xml', 'image/webp'].includes(a.media_type), 'only inert render assets are accepted');
    const item = await pinned(a.file); resources.set(url, { ...item, media_type: a.media_type }); refs.push({ url, ...a.file, media_type: a.media_type });
    if (a.media_type === 'text/css' && /local\s*\(/i.test(item.bytes.toString('utf8'))) localFontRules = true;
  }
  report.assets.push({ subject_id: s.id, document: s.document, extracted_html_sha256: sha(Buffer.from(html)), assets: refs });
  const blocked = [], served = [], consoleErrors = [];
  const context = await browser.newContext({ viewport: { width: s.viewport.width, height: s.viewport.height },
    deviceScaleFactor: s.viewport.device_scale_factor, javaScriptEnabled: false, offline: true,
    serviceWorkers: 'block', locale: 'zh-CN', timezoneId: 'Asia/Shanghai', colorScheme: 'light', acceptDownloads: false });
  const documentURL = new URL(s.document_url).href;
  await context.route('**/*', async route => {
    const url = route.request().url();
    if (url === documentURL && route.request().isNavigationRequest() && route.request().frame() === page.mainFrame()) {
      return route.fulfill({ status: 200, contentType: 'text/html', body: html, headers: {
        'content-security-policy': "default-src 'none'; script-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; form-action 'none'; style-src 'unsafe-inline' http: https: data:; font-src http: https: data:; img-src http: https: data:; base-uri http: https:"
      } });
    }
    const item = resources.get(url);
    if (item && ['stylesheet', 'font', 'image'].includes(route.request().resourceType())) { served.push(url); return route.fulfill({ status: 200, contentType: item.media_type, body: item.bytes }); }
    blocked.push({ url, type: route.request().resourceType() }); return route.abort('blockedbyclient');
  });
  const page = await context.newPage();
  page.on('console', message => { if (message.type() === 'error') consoleErrors.push(message.text()); });
  const response = await page.goto(documentURL, { waitUntil: 'load', timeout: 10000 }); need(response?.status() === 200, 'offline DOM navigation failed');
  await page.evaluate(() => document.fonts.ready);
  const facts = await page.evaluate(s => {
    const box = n => { const r = n.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; };
    const visible = n => {
      let r = n.getBoundingClientRect(); let left = Math.max(0, r.left), top = Math.max(0, r.top), right = Math.min(innerWidth, r.right), bottom = Math.min(innerHeight, r.bottom);
      for (let p = n; p; p = p.parentElement) {
        const c = getComputedStyle(p); if (c.display === 'none' || c.visibility === 'hidden' || c.visibility === 'collapse' || Number(c.opacity) === 0) return false;
        if (p !== n && /(hidden|clip|auto|scroll)/.test(c.overflowX + c.overflowY)) { const b = p.getBoundingClientRect(); left = Math.max(left, b.left); right = Math.min(right, b.right); top = Math.max(top, b.top); bottom = Math.min(bottom, b.bottom); }
      }
      return right > left && bottom > top && r.width > 0 && r.height > 0;
    };
    const px = v => /^\d+(?:\.\d+)?px$/.test(v) ? Number(v.slice(0, -2)) : null;
    const style = n => { const c = getComputedStyle(n); return { font_size_px: px(c.fontSize), line_height_px: px(c.lineHeight), line_height_css: c.lineHeight,
      font_family_css: c.fontFamily, font_weight_css: c.fontWeight, color: c.color, background_color: c.backgroundColor,
      display: c.display, visibility: c.visibility, opacity: Number(c.opacity), grid_template_columns: c.gridTemplateColumns, animation_name: c.animationName }; };
    const regions = {};
    for (const [key, selector] of Object.entries(s.regions)) {
      const nodes = [...document.querySelectorAll(selector)], n = nodes[0];
      if (nodes.length !== 1) { regions[key] = { selector, matches: nodes.length }; continue; }
      const plain = n.cloneNode(true); plain.querySelectorAll('rt,rp,script,style').forEach(x => x.remove());
      const base = getComputedStyle(n);
      const runs = []; const walker = document.createTreeWalker(n, NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) {
        const t = walker.currentNode, parent = t.parentElement;
        if (!t.textContent.trim() || parent.closest('rt,rp,script,style')) continue;
        const range = document.createRange(); range.selectNodeContents(t);
        const r = range.getBoundingClientRect();
        runs.push({ text: t.textContent, visible: visible(parent) && r.width > 0 && r.height > 0,
          rect: { x: r.x, y: r.y, width: r.width, height: r.height }, style: style(parent) });
      }
      const pos = [...n.querySelectorAll(s.pos_selector)].map(x => ({ text: x.textContent, marker: x.getAttribute('data-pos'), visible: visible(x), style: style(x),
        visual_effect: visible(x) && (getComputedStyle(x).color !== base.color || !['rgba(0, 0, 0, 0)', 'transparent'].includes(getComputedStyle(x).backgroundColor) || getComputedStyle(x).fontWeight !== base.fontWeight) }));
      regions[key] = { selector, matches: 1, text: plain.textContent.trim(), visible_text: runs.filter(r => r.visible).map(r => r.text).join('').trim(), text_runs: runs,
        visible: visible(n), rect: box(n), visibility_method: 'CSS ancestor and clipped geometric intersection; not pixel occlusion proof',
        client: { width: n.clientWidth, height: n.clientHeight, scroll_width: n.scrollWidth, scroll_height: n.scrollHeight }, style: style(n),
        ruby_count: n.querySelectorAll('ruby').length,
        rt: [...n.querySelectorAll('rt')].map(x => ({ text: x.textContent.trim(), visible: visible(x), rect: box(x), style: style(x) })),
        tokens: [...n.querySelectorAll(s.token_selector)].map(x => ({ text: (() => { const c = x.cloneNode(true); c.querySelectorAll('rt,rp').forEach(v => v.remove()); return c.textContent; })(), visible: visible(x), rect: box(x) })),
        pos, form_count: n.querySelectorAll('input,textarea,select').length };
    }
    return { document_url: location.href, browser_viewport: { width: innerWidth, height: innerHeight },
      document_client: { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight },
      document_scroll: { width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight, x: scrollX, y: scrollY },
      visual_viewport: visualViewport ? { width: visualViewport.width, height: visualViewport.height, scale: visualViewport.scale, x: visualViewport.offsetLeft, y: visualViewport.offsetTop } : null,
      device_pixel_ratio: devicePixelRatio, css_zoom: getComputedStyle(document.documentElement).zoom,
      media: s.media_queries.map(query => ({ query, matches: matchMedia(query).matches, parsed: matchMedia(query).media })),
      fonts: { status: document.fonts.status, faces: [...document.fonts].map(f => ({ family: f.family, status: f.status, weight: f.weight, style: f.style })) },
      form_count: document.querySelectorAll('input,textarea,select').length, active_tag_after_replay: document.activeElement?.tagName,
      running_animations: document.getAnimations().filter(a => a.playState === 'running').length,
      regions, desktop_viewport: null, native_window_rect: null, native_client_rect: null,
      dom_window_rect_kind: 'DOM element only; not a native client attestation' };
  }, s);
  const platformFonts = {};
  try {
    const cdp = await context.newCDPSession(page);
    await cdp.send('DOM.enable'); await cdp.send('CSS.enable');
    const document = await cdp.send('DOM.getDocument');
    for (const key of ['source', 'translation']) {
      const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: document.root.nodeId, selector: s.regions[key] });
      const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId });
      platformFonts[key] = fonts.sort((a, b) => a.familyName.localeCompare(b.familyName));
    }
    await cdp.detach();
  } catch (error) { platformFonts.error = error.message; }
  facts.platform_fonts = platformFonts;
  facts.capture = { kind: 'browser_viewport', full_page: false, native_attestation: false };
  const gaps = [];
  if (blocked.length) gaps.push('unavailable_or_unlisted_render_resource');
  if (consoleErrors.length) gaps.push('browser_reported_render_error');
  for (const [name, region] of Object.entries(facts.regions)) {
    if (region.matches !== 1) gaps.push('missing_or_duplicate_region:' + name);
    else if (!region.visible) gaps.push('region_not_visible:' + name);
    if (['source', 'translation'].includes(name) && region.matches === 1 && (region.style.font_size_px === null || region.style.line_height_px === null)) gaps.push('effective_css_px_unresolved:' + name);
    if (['source', 'translation'].includes(name) && region.matches === 1 && region.text_runs.some(r => r.visible && (r.style.font_size_px === null || r.style.line_height_px === null))) gaps.push('effective_text_run_css_px_unresolved:' + name);
  }
  if (facts.fonts.faces.some(f => f.status !== 'loaded')) gaps.push('font_not_loaded');
  if (platformFonts.error) gaps.push('platform_font_measurement_unavailable');
  if (s.evidence_kind === 'immutable_snapshot' && !s.assets.some(a => a.media_type.startsWith('font/'))) gaps.push('original_font_bytes_not_attested');
  if (s.evidence_kind === 'immutable_snapshot' && localFontRules) gaps.push('local_font_resolution_not_attested');
  if (s.evidence_kind === 'immutable_snapshot' && !s.assets.some(a => a.media_type.startsWith('font/') && served.includes(new URL(a.url).href))) gaps.push('pinned_font_bytes_not_loaded');
  if (s.evidence_kind === 'immutable_snapshot' && ['source', 'translation'].some(key => !platformFonts[key]?.length || platformFonts[key].some(f => !f.isCustomFont))) gaps.push('resolved_system_font_bytes_not_attested');
  if (facts.running_animations) gaps.push('animation_state_not_reconstructed');
  if (s.required_properties.form_properties) gaps.push('original_form_properties_not_reconstructed');
  if (s.required_properties.focus) gaps.push('original_focus_not_reconstructed');
  if (s.required_scope === 'whole_client') gaps.push('whole_client_reference_and_native_client_attestation_missing');
  if (s.evidence_kind === 'immutable_snapshot' && facts.form_count) gaps.push('snapshot_form_properties_not_attested');
  const screenshot = s.id + '.replay.png'; await page.screenshot({ path: path.join(out, screenshot), animations: 'allow' });
  const result = { id: s.id, evidence_kind: s.evidence_kind, status: gaps.length ? 'missing_render_evidence' : 'measured', requested_scope: s.required_scope,
    measured_scope: 'retained_dom_target', granted_product_scope: null, declared_state: s.declared_state ?? null,
    facts, gaps, served_assets: [...new Set(served)].sort(), blocked_requests: blocked, console_errors: consoleErrors,
    original_properties: { form: 'not_reconstructed', focus: 'not_reconstructed', animation_clock: 'not_reconstructed' },
    screenshot: { path: screenshot, sha256: await hashFile(path.join(out, screenshot)), bytes: (await fs.stat(path.join(out, screenshot))).size } };
  await context.close(); return result;
}

try {
  need(!rawOutput.includes('\\') && !rawOutput.split('/').includes('..'), 'unsafe lexical output path');
  need(out.startsWith(path.join(root, '.build') + path.sep) && !(await fs.stat(out).catch(() => null)), 'fresh ignored output required');
  let existingParent = path.dirname(out);
  while (!(await fs.lstat(existingParent).catch(() => null))) existingParent = path.dirname(existingParent);
  need(await fs.realpath(existingParent) === existingParent, 'symlink output ancestor forbidden');
  await fs.mkdir(path.dirname(out), { recursive: true });
  await fs.mkdir(out);
  outputCreated = true;
  const reqBytes = await fs.readFile(requestPath), req = JSON.parse(reqBytes);
  need(req.schema_version === 1 && req.kind === 'offline-render-request/v1' && /^[0-9a-f]{32}$/.test(req.nonce), 'typed request kind/version/nonce required');
  need(/^[0-9a-f]{64}$/.test(req.source_tree_sha256), 'exact managed source identity required');
  need(Array.isArray(req.subjects) && req.subjects.length > 0 && req.subjects.length <= 32 && new Set(req.subjects.map(s => s.id)).size === req.subjects.length, 'bounded unique subjects required');
  need(Array.isArray(req.pairs) && req.pairs.every(p => Array.isArray(p) && p.length === 2 && p.every(id => req.subjects.some(s => s.id === id))), 'known subject pairs required');
  report.nonce = req.nonce; report.request = { path: path.relative(root, requestPath), sha256: sha(reqBytes), bytes: reqBytes.length };
  report.collector = { path: path.relative(root, self), sha256: await hashFile(self) };
  report.source_before = tree(); need(report.source_before.sha256 === req.source_tree_sha256, 'requested managed source differs before measurement');
  const executable = chromium.executablePath(); report.environment.browser_executable = { path: executable, sha256: await hashFile(executable) };
  browser = await chromium.launch({ headless: true }); report.environment.browser_version = browser.version();
  for (const s of req.subjects) {
    try { report.subjects.push(await measure(s)); }
    catch (error) { report.subjects.push({ id: s.id, evidence_kind: s.evidence_kind, status: 'missing_render_evidence', gaps: [error.message], granted_product_scope: null }); }
  }
  for (const row of report.assets) { await pinned(row.document.file); for (const a of row.assets) await pinned(a); }
  report.source_after = tree(); need(report.source_after.sha256 === report.source_before.sha256, 'managed source changed during measurement');
  report.status = report.subjects.some(s => s.status !== 'measured') ? 'missing_render_evidence' : 'measured';
} catch (error) { report.errors.push(error.stack ?? error.message); }
finally {
  await browser?.close().catch(error => report.errors.push(error.message)); report.finished_at = new Date().toISOString();
  if (report.errors.length) report.status = 'failed';
  if (outputCreated) await fs.writeFile(path.join(out, 'measurement.json'), JSON.stringify(report, null, 2) + '\n');
}
console.log(JSON.stringify({ status: report.status, subjects: report.subjects.length, errors: report.errors, output: out, output_created: outputCreated }));
process.exitCode = report.status === 'measured' ? 0 : 1;
