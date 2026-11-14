/* Verify an exported report in a fresh, offline headless Chrome context. */
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const assert = require('node:assert/strict');
const { chromium } = require(path.resolve(__dirname, '../../.build/report-browser/node_modules/playwright-core'));

async function main() {
  const [reportArgument, outputArgument] = process.argv.slice(2);
  if (!reportArgument || !outputArgument) throw new Error('Usage: node scripts/report/verify_report.cjs <report-directory> <new-QA-directory>');
  const report = path.resolve(reportArgument);
  const output = path.resolve(outputArgument);
  fs.mkdirSync(output, { recursive: false });
  const browser = await chromium.launch({
    executablePath: process.env.REPORT_BROWSER_EXECUTABLE || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
    args: ['--disable-background-networking'],
  });
  const checks = [];
  const consoleErrors = [];
  const network = [];
  try {
    const context = await browser.newContext({ offline: true, viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
    const page = await context.newPage();
    page.on('pageerror', error => consoleErrors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') consoleErrors.push(message.text()); });
    page.on('request', request => { if (/^https?:/.test(request.url())) network.push(request.url()); });
    await page.goto(pathToFileURL(path.join(report, 'index.html')).href);
    assert.equal(await page.locator('.chapter').count(), 19);
    assert.equal(await page.locator('svg[role=img]').count(), 5);
    assert.equal(await page.locator('#requirements-table tbody tr').count(), 56);
    checks.push('offline_file_open_19_chapters_5_svg_56_requirements');
    await page.screenshot({ path: path.join(output, 'desktop-cover.png') });
    const targetFigure = page.locator('figure.figure').filter({ has: page.getByRole('heading', { name: '目标产品的组件与进程边界', exact: true }) });
    await targetFigure.screenshot({ path: path.join(output, 'system-architecture.png') });
    const threadFigure = page.locator('figure.figure').filter({ has: page.getByRole('heading', { name: '宿主线程、队列与控制状态', exact: true }) });
    await threadFigure.screenshot({ path: path.join(output, 'thread-model.png') });
    for (const [priority, count] of [['P0', 22], ['P1', 27], ['P2', 7]]) {
      await page.locator('#requirement-priority').selectOption(priority);
      assert.equal(await page.locator('#requirements-table tbody tr:visible').count(), count);
    }
    await page.locator('#requirement-priority').selectOption('');
    await page.locator('#requirement-search').fill('REQ-043');
    const selected = page.locator('#requirements-table tbody tr:visible');
    assert.equal(await selected.count(), 1);
    assert.equal(await selected.locator('td').nth(3).textContent(), 'S1');
    await page.locator('#requirement-search').fill('不存在的需求-qa');
    assert.equal(await page.locator('#requirements-table tbody tr:visible').count(), 0);
    assert.equal(await page.locator('#requirement-empty').isVisible(), true);
    await page.locator('#requirement-search').fill('');
    await page.locator('#requirement-package').selectOption('H1');
    assert.ok(await page.locator('#requirements-table tbody tr:visible').count() > 0);
    await page.locator('#requirement-package').selectOption('');
    checks.push('requirements_search_priority_package_and_empty_state');
    await page.getByRole('button', { name: 'CrossOver', exact: true }).click();
    assert.equal(await page.locator('.platform-card.dim').count(), 2);
    await page.getByRole('button', { name: '全部环境', exact: true }).click();
    assert.equal(await page.locator('.platform-card.dim').count(), 0);
    checks.push('platform_focus_and_reset');
    await page.getByRole('button', { name: '深色阅读', exact: true }).click();
    assert.equal(await page.locator('body').evaluate(element => element.classList.contains('dark')), true);
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({ path: path.join(output, 'desktop-dark.png') });
    await page.getByRole('button', { name: '浅色阅读', exact: true }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => window.scrollTo(0, 0));
    const widths = await page.evaluate(() => ({ page: document.documentElement.scrollWidth, viewport: document.documentElement.clientWidth }));
    assert.ok(widths.page <= widths.viewport + 1, `mobile page overflow: ${JSON.stringify(widths)}`);
    await page.screenshot({ path: path.join(output, 'mobile-cover.png') });
    await page.locator('#requirements').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, 'mobile-requirements.png') });
    checks.push('theme_toggle_mobile_390_no_page_overflow');
    await page.locator('#requirement-priority').selectOption('P0');
    await page.emulateMedia({ media: 'print' });
    assert.equal(await page.locator('.sidebar').isVisible(), false);
    assert.equal(await page.locator('#requirements-table tbody tr:visible').count(), 56);
    await page.pdf({ path: path.join(output, 'print-preview.pdf'), format: 'A4', printBackground: true });
    await page.emulateMedia({ media: 'screen' });
    checks.push('print_all_56_rows_even_when_filtered');
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(pathToFileURL(path.join(report, 'gallery.html')).href + '?screen=SCR-05&variant=light');
    await page.locator('#preview').evaluate(image => image.decode());
    assert.match(await page.locator('#title').textContent(), /SCR-05/);
    assert.equal(await page.locator('#screens button').count(), 22);
    const images = await page.evaluate(async () => {
      const previews = manifest.screens.flatMap(screen => screen.previews);
      const loaded = await Promise.all(previews.map(preview => new Promise(resolve => {
        const image = new Image();
        image.onload = () => resolve(true);
        image.onerror = () => resolve(false);
        image.src = 'references/docs/product/original/design/' + preview.file;
      })));
      return { total: loaded.length, loaded: loaded.filter(Boolean).length };
    });
    assert.deepEqual(images, { total: 89, loaded: 89 });
    await page.locator('#filter').fill('SCR-22');
    assert.equal(await page.locator('#screens button').count(), 1);
    await page.locator('#screens button').click();
    await page.locator('#preview').evaluate(image => image.decode());
    assert.match(await page.locator('#title').textContent(), /SCR-22/);
    await page.screenshot({ path: path.join(output, 'offline-gallery.png') });
    checks.push('offline_gallery_all_89_images_and_22_screen_navigation');
    assert.deepEqual(consoleErrors, []);
    assert.deepEqual(network, []);
    const result = { passed: true, mode: 'file:// with offline browser context', browser: await browser.version(), checks, console_errors: consoleErrors, http_requests: network, image_loading: images, mobile_width: widths };
    fs.writeFileSync(path.join(output, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
    await context.close();
  } finally {
    await browser.close();
  }
}

main().catch(error => { process.stderr.write(error.stack + '\n'); process.exitCode = 1; });
