import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath, pathToFileURL} from 'node:url';
import test from 'node:test';
import {chromium} from 'playwright-core';
import {PNG} from 'pngjs';
import {compareImages} from './visual-compare.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('visual: identical images pass; wrong size fails', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'luna-visual-'));
  try {
    const png = new PNG({width: 12, height: 12}); png.data.fill(128);
    const baseline = path.join(dir, 'baseline.png'); fs.writeFileSync(baseline, PNG.sync.write(png));
    assert.equal(compareImages({baseline, candidate: baseline, diff: path.join(dir, 'same.png')}).status, 'passed');
    const candidate = path.join(dir, 'small.png'); fs.writeFileSync(candidate, PNG.sync.write(new PNG({width: 10, height: 12})));
    assert.equal(compareImages({baseline, candidate, diff: path.join(dir, 'size.png')}).reason, 'dimensions');
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});

test('design violation: actual Playwright screenshot rejects changed material color', async () => {
  const out = path.join(root, '.build/quality/visual-adversarial'); fs.mkdirSync(out, {recursive: true});
  const chrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const browser = await chromium.launch({headless: true, ...(fs.existsSync(chrome) ? {executablePath: chrome} : {})});
  const errors = [], http = [];
  try {
    const context = await browser.newContext({offline: true, viewport: {width: 1462, height: 1100}, deviceScaleFactor: 1, reducedMotion: 'reduce'});
    const page = await context.newPage(); page.on('pageerror', e => errors.push(e.message));
    page.on('request', request => { if (/^https?:/.test(request.url())) http.push(request.url()); });
    await page.goto(pathToFileURL(path.join(root, 'docs/design/macos-material-case-v1/index.html')).href);
    await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.images].map(img => img.decode())); });
    const target = page.locator('#material-window');
    const baseline = path.join(out, 'approved-source-render.png');
    const candidate = path.join(out, 'unchanged-render.png');
    await target.screenshot({path: baseline}); await target.screenshot({path: candidate});
    assert.equal(compareImages({baseline, candidate, diff: path.join(out, 'unchanged-diff.png')}).status, 'passed');
    // Mutate the browser rendering only. Never overwrite a design or approved preview file.
    await page.addStyleTag({content: '.material-surface{background:#ff00c8!important;backdrop-filter:none!important}'});
    const wrong = path.join(out, 'wrong-material.png'); await target.screenshot({path: wrong});
    const diff = path.join(out, 'wrong-material-diff.png');
    const result = spawnSync(process.execPath, [path.join(root, 'scripts/quality/visual-compare.mjs'), '--baseline', baseline, '--candidate', wrong, '--diff', diff], {encoding: 'utf8'});
    assert.equal(result.status, 1, result.stderr);
    assert.ok(JSON.parse(result.stdout).ratio > 0.1);
    assert.deepEqual(errors, []); assert.deepEqual(http, []);
    fs.writeFileSync(path.join(out, 'verification.json'), JSON.stringify({unchanged: 'passed', injected_wrong_material: 'rejected', rejection_exit_code: result.status, comparison: JSON.parse(result.stdout), scope: 'same-run replay of approved material source; verifies the guard, not a new product visual acceptance'}, null, 2) + '\n');
  } finally { await browser.close(); }
});
