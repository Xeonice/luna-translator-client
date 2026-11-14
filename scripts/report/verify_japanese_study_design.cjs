/* Capture and check a design proposal; no tokenizer, dictionary or model is run. */
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const { chromium } = require(path.resolve(__dirname, '../../.build/report-browser/node_modules/playwright-core'));

async function main() {
  const [source, destination] = process.argv.slice(2);
  if (!source || !destination) throw new Error('Usage: node scripts/report/verify_japanese_study_design.cjs <prototype.html> <new-output-directory>');
  const input = path.resolve(source), output = path.resolve(destination);
  const revision = path.basename(path.dirname(input));
  fs.mkdirSync(output, { recursive: false });
  const browser = await chromium.launch({ executablePath: process.env.REPORT_BROWSER_EXECUTABLE || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--disable-background-networking'] });
  const errors = [], network = [], checks = [], captures = [];
  try {
    const context = await browser.newContext({ offline: true, viewport: { width: 1440, height: 1100 }, reducedMotion: 'reduce' });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('request', request => { if (/^https?:/.test(request.url())) network.push(request.url()); });
    const open = async query => { await page.goto(pathToFileURL(input).href + '?' + query); await page.evaluate(() => document.fonts.ready); };
    const capture = async (name, selector) => {
      await page.locator(selector).screenshot({ path: path.join(output, name), animations: 'disabled' });
      captures.push(name);
    };
    const sourceText = () => page.locator('#source-after').evaluate(element => {
      const clone = element.cloneNode(true); clone.querySelectorAll('rt,rp').forEach(node => node.remove()); return clone.textContent;
    });
    await open('view=reading&theme=light&scene=day');
    const initialSourceSize = await page.locator('#source-after').evaluate(element => parseFloat(getComputedStyle(element).fontSize));
    const hasPopover = await page.locator('#word-popover').count() > 0;
    const sceneStyles = () => page.evaluate(() => ({ filter: getComputedStyle(document.querySelector('.scene-art')).filter, shade: getComputedStyle(document.querySelector('.scene-shade')).backgroundImage }));
    const brightScene = hasPopover ? await sceneStyles() : null;
    assert.equal(await page.locator('#segmentation').isChecked(), true);
    assert.equal(await page.locator('#furigana').isChecked(), true);
    assert.equal(await page.locator('#pos-colors').isChecked(), true);
    assert.ok(await page.locator('#source-after ruby').count() > 0);
    assert.equal(await sourceText(), await page.locator('#source-before').textContent());
    assert.equal(await page.locator('.hud-controls').first().evaluate(element => Math.round(element.getBoundingClientRect().height)), 32);
    await capture('01-reading-light.png', '#view-reading');
    await page.locator('#source-after button').filter({ hasText: '図書館' }).click();
    assert.equal(await page.locator('#word-surface').textContent(), '図書館');
    assert.match(await page.locator('#word-reading').textContent(), /としょかん/);
    await capture('02-word-card.png', '.word-card');
    if (hasPopover) {
      assert.equal(await page.locator('#word-popover').isVisible(), true);
      await capture('02b-anchored-word-card.png', '.stage');
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('#word-popover').isVisible(), false);
      assert.equal(await page.locator('#source-after [data-token="2"]').evaluate(element => element === document.activeElement), true);
      await page.locator('#source-after [data-token="2"]').click();
      await page.locator('#view-reading .view-heading h2').click();
      assert.equal(await page.locator('#word-popover').isVisible(), false);
      await page.locator('#source-after [data-token="2"]').click();
      await page.locator('#full-dictionary').click();
      assert.equal(await page.locator('#learning-panel').isVisible(), true);
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('#learning-panel').isVisible(), false);
      checks.push('anchored_popover_escape_outside_close_focus_return_and_learning_panel');
    }
    await page.locator('#segmentation').uncheck();
    assert.equal(await page.locator('#source-after ruby').count(), 0);
    assert.equal(await sourceText(), await page.locator('#source-before').textContent());
    await page.locator('#segmentation').check();
    await page.locator('#furigana').uncheck();
    assert.equal(await page.locator('#source-after ruby').count(), 0);
    await page.locator('#furigana').check();
    await page.locator('#pos-colors').uncheck();
    assert.equal(await page.locator('#source-after .pos-on').count(), 0);
    checks.push('default_enabled_ruby_pos_word_selection_plaintext_and_independent_switches');
    await open('view=reading&theme=dark&scene=day');
    await capture('03-reading-dark.png', '#view-reading');
    if (hasPopover) assert.deepEqual(await sceneStyles(), brightScene, 'HUD appearance must not darken the fixed bright preview scene');
    await open('view=analysis');
    await page.locator('.clause').filter({ hasText: '図書館で' }).click();
    const highlighted = await page.locator('#analysis-source .token.is-referenced').evaluateAll(elements => elements.map(el => el.cloneNode(true)).map(el => { el.querySelectorAll('rt,rp').forEach(node => node.remove()); return el.textContent; }).join(''));
    assert.equal(highlighted, '図書館で');
    await capture('04-grammar-explanation.png', '#view-analysis');
    await page.locator('[data-analysis-state="loading"]').click();
    assert.equal(await page.locator('#analysis-placeholder').isVisible(), true);
    await page.getByRole('button', { name: '取消分析', exact: true }).click();
    await page.locator('[data-analysis-state="failure"]').click();
    assert.match(await page.locator('#analysis-placeholder').textContent(), /暂时没有响应/);
    await capture('05-grammar-failure.png', '#view-analysis');
    await page.locator('[data-analysis-state="stale"]').click();
    assert.equal(await page.locator('#latest-sentence-notice').isVisible(), true);
    await capture('06-grammar-frozen-sentence.png', '#view-analysis');
    checks.push('grammar_ready_loading_cancel_failure_and_frozen_sentence_design_states');
    await open('view=settings&settings=learning');
    assert.equal(await page.locator('.settings-route').count(), 10);
    if (hasPopover) assert.equal(await page.locator('#settings-source button').count(), 0, 'Settings preview should not open a popover in a hidden reading view');
    assert.equal(await page.locator('#resource-state').count(), 0);
    assert.match(await page.locator('#settings-learning').textContent(), /内置日语词典/);
    await capture('07-learning-settings.png', '#view-settings');
    await page.locator('#kana-mode').selectOption('katakana');
    assert.ok(await page.locator('#settings-source rt').allTextContents().then(items => items.some(text => /[ァ-ヶ]/.test(text))));
    await open('view=settings&settings=services');
    await capture('08-model-services.png', '#view-settings');
    await page.locator('#test-connection').click();
    assert.match(await page.locator('#toast').textContent(), /未测试/);
    checks.push('ten_settings_routes_bundled_dictionary_no_install_flow_kana_and_no_fake_connection_success');
    await open('view=reading&sentence=long&scale=2');
    assert.equal(await sourceText(), await page.locator('#source-before').textContent());
    const textSize = await page.locator('#source-after').evaluate(element => getComputedStyle(element).fontSize);
    assert.equal(textSize, `${initialSourceSize * 2}px`);
    await capture('09-long-text-200-percent.png', '#view-reading');
    if (hasPopover) {
      await page.waitForFunction(() => { const area = document.querySelector('#hud-reading-area'); return area.scrollHeight > area.clientHeight; }, undefined, { timeout: 2000 });
      assert.equal(await page.locator('#scroll-indicator').isVisible(), true);
      await page.locator('#hud-reading-area').evaluate(element => { element.scrollTop = element.scrollHeight; });
      await capture('09b-long-text-scrolled.png', '.stage');
      assert.equal(await page.locator('#hud-after .hud-controls').evaluate(element => Math.round(element.getBoundingClientRect().height)), 32);
      await open('view=reading&theme=light&preset=compact&scene=day');
      assert.equal(await page.locator('#hud-after').evaluate(element => Math.round(element.getBoundingClientRect().width)), 500);
      assert.ok(await page.locator('#source-after rt').evaluateAll(elements => elements.every(element => parseFloat(getComputedStyle(element).fontSize) >= 10)));
      await capture('10-compact-reading.png', '#view-reading');
      await open('view=reading&theme=light&appearance=reduced&scene=day');
      const surface = await page.locator('#hud-after').evaluate(element => ({ backdrop: getComputedStyle(element).backdropFilter, color: getComputedStyle(element).backgroundColor }));
      assert.equal(surface.backdrop, 'none');
      assert.match(surface.color, /^rgb\(/);
      await capture('11-reduced-transparency.png', '#view-reading');
      await page.locator('#high-contrast').check();
      await capture('12-increased-contrast.png', '#view-reading');
      checks.push('fixed_bright_scene_light_dark_compact_ruby_minimum_scroll_affordance_opaque_fallback_and_contrast');
    }
    checks.push('long_text_200_percent_preserves_source_and_control_row');
    await page.setViewportSize({ width: 390, height: 844 });
    for (const query of ['view=reading', 'view=analysis', 'view=settings&settings=learning', 'view=settings&settings=services']) {
      await open(query);
      const widths = await page.evaluate(() => ({ document: document.documentElement.scrollWidth, viewport: document.documentElement.clientWidth }));
      assert.ok(widths.document <= widths.viewport + 1, `Document overflow (${query}): ${JSON.stringify(widths)}`);
    }
    checks.push('all_four_views_390_no_document_overflow');
    assert.deepEqual(errors, []); assert.deepEqual(network, []);
    const result = { revision, artifact_qa_passed: true, user_review: 'not_reviewed', runtime_tests: 'NOT_RUN', real_tokenizer_executed: false, real_model_executed: false, product_regression_executed: false, mode: 'offline file:// design preview', prototype_sha256: crypto.createHash('sha256').update(fs.readFileSync(input)).digest('hex'), browser: await browser.version(), checks, captures, console_errors: errors, http_requests: network };
    fs.writeFileSync(path.join(output, 'verification.json'), JSON.stringify(result, null, 2) + '\n');
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
    await context.close();
  } finally { await browser.close(); }
}
main().catch(error => { process.stderr.write(error.stack + '\n'); process.exitCode = 1; });
