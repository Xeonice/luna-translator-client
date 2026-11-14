/** Functional and visual runners share an exact managed browser, never local Chrome. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const require = createRequire(path.join(root, 'package.json'));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
export async function launchLockedBrowser() {
  const { chromium } = require('playwright-core');
  const installed = require('playwright-core/package.json').version;
  const packagePath = require.resolve('playwright-core/package.json');
  const manifest = JSON.parse(await fs.readFile(path.join(path.dirname(packagePath), 'browsers.json'), 'utf8'));
  const packageJson = JSON.parse(await fs.readFile(path.join(root, 'package.json'), 'utf8'));
  const lock = JSON.parse(await fs.readFile(path.join(root, 'package-lock.json'), 'utf8'));
  assert.equal(installed, packageJson.devDependencies['playwright-core']);
  assert.equal(installed, lock.packages['node_modules/playwright-core'].version);
  const executable = chromium.executablePath();
  const identity = { playwright: installed, executable, binarySha256: hash(await fs.readFile(executable)), version: manifest.browsers.find(item => item.name === 'chromium').browserVersion };
  // Explicitly run the binary whose bytes are attested. Playwright may otherwise
  // choose a separate headless-shell executable when no path is supplied.
  const browser = await chromium.launch({ executablePath: executable, headless: true, args: ['--disable-gpu', '--force-color-profile=srgb'] });
  try { assert.equal(browser.version(), identity.version); }
  catch (error) { await browser.close(); throw error; }
  return { browser, identity };
}
export async function verifyBrowserIdentity(identity) {
  assert.equal(hash(await fs.readFile(identity.executable)), identity.binarySha256, 'Managed browser changed during the run');
}
