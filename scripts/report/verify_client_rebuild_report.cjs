/* Verify the offline planning artifact; this is not a product/game regression. */
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { pathToFileURL, fileURLToPath } = require('node:url');
const { chromium } = require(path.resolve(__dirname, '../../.build/report-browser/node_modules/playwright-core'));

async function main() {
  const [source, destination] = process.argv.slice(2);
  if (!source || !destination) throw new Error('Usage: node scripts/report/verify_client_rebuild_report.cjs <report-directory> <new-QA-directory>');
  const report = path.resolve(source);
  const output = path.resolve(destination);
  const manifest = JSON.parse(fs.readFileSync(path.join(report, 'report-manifest.json'), 'utf8'));
  fs.mkdirSync(output, { recursive: false });
  const browser = await chromium.launch({ executablePath: process.env.REPORT_BROWSER_EXECUTABLE || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--disable-background-networking'] });
  const errors = [], network = [], checks = [];
  try {
    const context = await browser.newContext({ offline: true, viewport: { width: 1440, height: 1000 } });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('request', request => { if (/^https?:/.test(request.url())) network.push(request.url()); });
    await page.goto(pathToFileURL(path.join(report, 'index.html')).href);
    assert.equal(await page.locator('.chapter').count(), manifest.sections || 7);
    assert.equal(await page.locator('svg[role="img"]').count(), 1);
    assert.equal(await page.locator('#tasks-table tbody tr').count(), manifest.validation.tasks);
    assert.equal(await page.locator('#cases-table tbody tr').count(), manifest.validation.regression_cases);
    assert.equal(await page.locator('#requirements-table tbody tr').count(), manifest.validation.requirements_mapped);
    assert.ok(await page.locator('.scope').textContent().then(text => text.includes('本轮仅规划')));
    const localLinks = await page.locator('a[href]').evaluateAll(links => links.map(link => link.href).filter(href => href.startsWith('file:')));
    for (const href of localLinks) {
      const url = new URL(href);
      assert.ok(fs.existsSync(fileURLToPath(url)), `Missing report reference: ${href}`);
      if (url.pathname.endsWith('/index.html') && url.hash) {
        assert.equal(await page.locator(url.hash).count(), 1, `Missing anchor: ${url.hash}`);
      }
    }
    checks.push('planning_scope_counts_and_all_html_reference_links');
    await page.screenshot({ path: path.join(output, 'desktop-cover.png') });
    await page.locator('svg[role="img"]').screenshot({ path: path.join(output, 'planned-process-boundary.png') });
    await page.locator('input[data-table="cases-table"]').fill('CR-03-N7');
    assert.equal(await page.locator('#cases-table tbody tr:visible').count(), 1);
    await page.locator('input[data-table="cases-table"]').fill('不存在的回归项');
    assert.equal(await page.locator('#cases-table tbody tr:visible').count(), 0);
    await page.emulateMedia({ media: 'print' });
    assert.equal(await page.locator('#cases-table tbody tr:visible').count(), manifest.validation.regression_cases);
    await page.emulateMedia({ media: 'screen' });
    await page.locator('input[data-table="cases-table"]').fill('');
    checks.push('csv_table_search_empty_result_and_print_restores_rows');
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => window.scrollTo(0, 0));
    const widths = await page.evaluate(() => ({ document: document.documentElement.scrollWidth, viewport: document.documentElement.clientWidth }));
    assert.ok(widths.document <= widths.viewport + 1, `Mobile page overflow: ${JSON.stringify(widths)}`);
    await page.screenshot({ path: path.join(output, 'mobile-cover.png') });
    checks.push('mobile_390_no_document_overflow');
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(pathToFileURL(path.join(report, 'gallery.html')).href);
    assert.equal(await page.locator('.chapter').count(), manifest.validation.design_screens);
    const images = await page.locator('img').evaluateAll(async elements => {
      const outcomes = await Promise.all(elements.map(async img => { img.loading = 'eager'; try { await img.decode(); return img.naturalWidth > 0; } catch { return false; } }));
      return { count: outcomes.length, loaded: outcomes.filter(Boolean).length };
    });
    assert.deepEqual(images, { count: 89, loaded: 89 });
    await page.screenshot({ path: path.join(output, 'offline-design-gallery.png') });
    checks.push('file_gallery_22_screens_and_all_89_original_images');
    assert.deepEqual(network, []);
    assert.deepEqual(errors, []);
    const result = { artifact_qa_passed: true, product_regression_executed: false, mode: 'offline file://', browser: await browser.version(), checks, images, mobile_widths: widths, console_errors: errors, http_requests: network };
    fs.writeFileSync(path.join(output, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
    await context.close();
  } finally {
    await browser.close();
  }
}
main().catch(error => { process.stderr.write(error.stack + '\n'); process.exitCode = 1; });
