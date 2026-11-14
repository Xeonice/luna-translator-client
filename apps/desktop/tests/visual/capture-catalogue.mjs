import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { assertCaptureConditions } from './capture-conditions.mjs';
import { CATALOGUE_PATH, loadDesignSourceVersion } from './design-source-versions.mjs';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

export const HISTORICAL_CAPTURE_BLOCKS = Object.freeze({
  'session-current-body': 'retired_session_tabs',
  'session-current-extraction': 'retired_session_tabs',
  'session-current-hook-events': 'retired_session_tabs',
  'preview-off': 'retired_manual_preview_off',
  'target-review-preview-off': 'retired_manual_preview_off',
});
export const BLOCKED_REFERENCE_STATUS = 'blocked_pending_replacement_reference';

export function historicalCaptureBlock(catalogue, id) {
  const code = catalogue.schemaVersion === 4 ? HISTORICAL_CAPTURE_BLOCKS[id] : null;
  if (!code) return null;
  return { code, replacementReference: null, reason: code === 'retired_session_tabs'
    ? 'The old session Tab composition has been retired. The original ID, variants and PNGs remain required; independent replacement references are pending.'
    : 'The manual-enable Off premise has been retired: a valid target with granted permission starts preview automatically. The original ID, variants and PNGs remain required; independent replacement references are pending.' };
}

// Array order, length, selectors, top and both dimensions remain exact. Only
// diagnostics are separated; extra or missing fields are failures as before.
export function scrollConditionDifferences(expected, actual) {
  const differences = [];
  if (!Array.isArray(actual)) return [{ field: 'scrollRegions', expected, actual: actual ?? null }];
  if (expected.length !== actual.length) differences.push({ field: 'scrollRegions.length', expected: expected.length, actual: actual.length });
  for (let index = 0; index < Math.max(expected.length, actual.length); index++) {
    const before = expected[index], after = actual[index];
    if (!before || !after) { differences.push({ index, field: 'scrollRegion', expected: before ?? null, actual: after ?? null }); continue; }
    for (const field of new Set([...Object.keys(before), ...Object.keys(after)])) {
      if (JSON.stringify(before[field]) !== JSON.stringify(after[field])) differences.push({ index, field, expected: before[field] ?? null, actual: after[field] ?? null });
    }
  }
  return differences;
}

export function assertReviewedSessionQuery(actual, expected) {
  assert.deepEqual(actual, expected, 'Actual session/dataset/filter/order/pageSize/cursor query differs from the independent fixture');
  return structuredClone(actual);
}

export async function loadReferenceCatalogue(file, { root, proposalPreview = false }) {
  const pathname = path.resolve(root, file), bytes = await fs.readFile(pathname);
  const catalogue = JSON.parse(bytes);
  assert.ok([3, 4].includes(catalogue.schemaVersion), 'Unsupported independent reference catalogue schema');
  if (!proposalPreview) assert.equal(catalogue.status, 'registered_independently_reviewed', 'Unreviewed design proposals cannot pass product acceptance');
  assert.deepEqual(catalogue.pixelmatch, { threshold: .1, includeAA: true, maxRatio: 0 });
  const registeredRoot = catalogue.schemaVersion === 4 ? 'docs/design/first-product-v1/acceptance-v4/' : 'docs/design/first-product-v1/acceptance-v3/';
  const sourceVersion = catalogue.schemaVersion === 4 && !proposalPreview
    ? (assert.equal(path.relative(root, pathname), CATALOGUE_PATH, 'Historical resolution requires the exact registered v4 catalogue'), await loadDesignSourceVersion(root, catalogue, hash(bytes))) : null;
  const resolveReference = relative => sourceVersion ? sourceVersion.resolve(relative)
    : proposalPreview && relative.startsWith(registeredRoot)
      ? path.join(path.dirname(pathname), relative.slice(registeredRoot.length)) : path.resolve(root, relative);
  for (const [relative, expected] of Object.entries(catalogue.frozenFiles)) assert.equal(hash(await fs.readFile(resolveReference(relative))), expected, `Frozen design input changed: ${relative}`);
  return { ...catalogue, file: path.relative(root, pathname), sha256: hash(bytes), proposalPreview, resolveReference,
    designSourceVersion: sourceVersion?.receipt ?? null };
}

async function setUnderlay(page, definition, catalogue) {
  if (!definition) return null;
  const bytes = await fs.readFile(catalogue.resolveReference(definition.file));
  assert.equal(hash(bytes), definition.sha256);
  const data = `data:image/png;base64,${bytes.toString('base64')}`;
  await page.evaluate(async ({ data, definition }) => {
    const background = document.querySelector('[data-ui-underlay]');
    if (!background) throw new Error('Missing controlled material-input host');
    background.style.backgroundImage = `url("${data}")`;
    background.style.backgroundColor = '#020c31';
    background.style.backgroundRepeat = 'no-repeat';
    background.style.backgroundSize = definition.placement === 'center / cover' ? 'cover' : `${definition.size[0]}px ${definition.size[1]}px`;
    background.style.backgroundPosition = definition.placement === 'center / cover' ? 'center' : `${-definition.sampleOrigin[0]}px ${-definition.sampleOrigin[1]}px`;
    const image = new Image(); image.src = data; await image.decode();
    if (!getComputedStyle(background).backgroundImage.includes(data)) throw new Error('Material input was not applied');
  }, { data, definition });
  return structuredClone(definition);
}

async function prepareVariant(page, state, variant) {
  if (JSON.stringify(page.viewportSize()) !== JSON.stringify(variant.viewport)) await page.setViewportSize(variant.viewport);
  const preferences = variant.requiredConditions.preferences;
  if (preferences) await page.evaluate(preferences => {
    const port = window.uiHarness;
    if (Object.entries(preferences).some(([key, value]) => port.state.preferences[key] !== value)) port.publish({ preferences: { ...port.state.preferences, ...preferences } });
  }, preferences);
  if (state.id === 'dictionary-distinct-headwords') {
    const details = page.locator('[data-word-card-surface] details');
    for (const [index, expected] of (variant.details ?? []).entries()) {
      const detail = details.nth(index);
      if (await detail.evaluate(node => node.open) !== expected.open) await detail.locator('summary').click();
    }
  }
  if (state.id === 'glossary-editing') {
    const target = variant.requiredConditions.fields?.find(field => field.name === '译法 1');
    if (target && await page.getByRole('textbox', { name: '译法 1', exact: true }).inputValue() !== target.value) {
      await page.getByRole('textbox', { name: '译法 1', exact: true }).fill(target.value);
    }
    if (variant.designState?.error) {
      await page.evaluate(() => { window.uiHarness.failNextCommand = true; });
      await page.getByRole('button', { name: '保存草稿', exact: true }).click();
      await page.getByRole('alert').waitFor();
    }
  }
  for (const scroll of variant.scrollTop ?? []) {
    const node = page.locator(scroll.selector).filter({ visible: true });
    assert.equal(await node.count(), 1, `Scrollable surface must be unambiguous: ${scroll.selector}`);
    await node.evaluate((element, top) => { element.scrollTop = top; }, scroll.top);
  }
  await page.mouse.move(0, 0);
  await page.evaluate(async () => { const dialog=document.querySelector('[role=dialog]');if(dialog&&dialog.tabIndex===-1)dialog.focus({preventScroll:true});else document.activeElement?.blur?.(); await document.fonts.ready; await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); });
}

export async function captureCatalogueState({ page, id, legacyReference, catalogue, root, out }) {
  const definition = catalogue.states.find(state => state.id === id);
  assert.ok(definition, `Missing registered capture state: ${id}`);
  const blocked = historicalCaptureBlock(catalogue, id);
  if (blocked) {
    const rows = definition.variants.map(variant => ({ id, variantId: variant.variantId, status: BLOCKED_REFERENCE_STATUS,
      reference: variant.reference, referenceSha256: variant.referenceSha256, expectedSize: variant.expectedSize,
      blocked, reason: blocked.reason }));
    const [main, ...variants] = rows;
    return { ...main, variants, catalogue: catalogue.file, catalogueSha256: catalogue.sha256, proposalPreview: catalogue.proposalPreview };
  }
  const require = createRequire(path.join(root, 'package.json'));
  const { PNG } = require('pngjs'), pixelmatch = require('pixelmatch').default;
  const results = [];
  for (const variant of definition.variants) {
    await prepareVariant(page, definition, variant);
    const underlay = await setUnderlay(page, definition.underlay, catalogue);
    const verifiedConditions = await assertCaptureConditions(page, variant.requiredConditions);
    const query = definition.dataState?.componentInputs?.supplement?.query;
    const sessionRecordQuery = query ? assertReviewedSessionQuery(await page.evaluate(expected => window.uiHarness.sessionRecordQueries.filter(actual => actual.sessionId === expected.sessionId && actual.dataset === expected.dataset).at(-1), query), query) : null;
    // Conditions exercise real Select options and Escape. Restore the declared
    // unfocused design composition after that observation, without changing a
    // popup/focus-specific state or hiding its accessibility focus indicator.
    if (variant.requiredConditions.canonicalFocus === 'neutral' && variant.selector !== '[data-select-panel]') { if (!await page.locator('[role=dialog]:visible').count()) await page.mouse.click(1,1); await page.evaluate(async () => { const dialog=document.querySelector('[role=dialog]');if(dialog&&dialog.tabIndex===-1)dialog.focus({preventScroll:true});else document.activeElement?.blur?.();await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))); }); await page.mouse.move(0,0); }
    const scrollTop = [];
    for (const expected of variant.scrollTop ?? []) {
      const actual = await page.locator(expected.selector).filter({ visible: true }).evaluate(element => ({ top: element.scrollTop, clientHeight: element.clientHeight, scrollHeight: element.scrollHeight }));
      scrollTop.push({ selector: expected.selector, ...actual });
    }
    const bounds = await page.locator(variant.selector).boundingBox();
    const focusState=await page.evaluate(()=>{const n=document.activeElement;return{tag:n?.tagName?.toLowerCase(),role:n?.getAttribute('role'),label:n?.getAttribute('aria-label'),focusVisible:!!n?.matches(':focus-visible')};});
    if (variant.requiredConditions.canonicalFocus === 'neutral') { assert.equal(focusState.focusVisible, false, `Canonical independent design capture must remain unfocused after control observations: ${JSON.stringify(focusState)} (${id}/${variant.variantId})`); verifiedConditions.canonicalFocus='neutral'; verifiedConditions.focusState=focusState; }
    const prefix = `${id}.${variant.variantId}`, candidate = `${prefix}.candidate.png`, baselineCopy = `${prefix}.baseline.png`, diff = `${prefix}.diff.png`;
    await page.locator(variant.selector).screenshot({ path: path.join(out, candidate), animations: 'disabled' });
    const referenceBytes = await fs.readFile(catalogue.resolveReference(variant.reference));
    assert.equal(hash(referenceBytes), variant.referenceSha256);
    await fs.writeFile(path.join(out, baselineCopy), referenceBytes);
    const candidateBytes = await fs.readFile(path.join(out, candidate));
    const expected = PNG.sync.read(referenceBytes), actual = PNG.sync.read(candidateBytes);
    const width = Math.max(expected.width, actual.width), height = Math.max(expected.height, actual.height);
    const before = new PNG({ width, height }), after = new PNG({ width, height }); before.data.fill(0); after.data.fill(0);
    PNG.bitblt(expected, before, 0, 0, expected.width, expected.height, 0, 0); PNG.bitblt(actual, after, 0, 0, actual.width, actual.height, 0, 0);
    const diffImage = new PNG({ width, height });
    const differentPixels = pixelmatch(before.data, after.data, diffImage.data, width, height, { threshold: .1, includeAA: true });
    const diffBytes = PNG.sync.write(diffImage); await fs.writeFile(path.join(out, diff), diffBytes);
    const scrollDifferences = scrollConditionDifferences(variant.scrollTop ?? [], scrollTop);
    const conditionsMatch = scrollDifferences.length === 0;
    const geometryMatches = JSON.stringify(bounds) === JSON.stringify(variant.expectedBounds);
    const referencePremiseMismatch = catalogue.schemaVersion === 4 && id === 'target-review'
      ? { field: 'previewActivation', expected: 'manual_enable_off', actual: 'automatic_start_with_valid_target_and_granted_permission' } : null;
    results.push({ id, variantId: variant.variantId, status: differentPixels === 0 && expected.width === actual.width && expected.height === actual.height && conditionsMatch && geometryMatches && !referencePremiseMismatch ? 'passed' : 'failed', reference: variant.reference, referenceSha256: variant.referenceSha256, legacyReference, baselineCopy, baselineCopySha256: hash(referenceBytes), candidate, candidateSha256: hash(candidateBytes), diff, diffSha256: hash(diffBytes), expectedSize: [expected.width, expected.height], actualSize: [actual.width, actual.height], differentPixels, ratio: differentPixels / (width * height), pixelmatchThreshold: .1, includeAA: true, maxRatio: 0, captureConditions: { viewport: page.viewportSize(), selector: variant.selector, bounds, deviceScaleFactor: 1, scrollTop, scrollDifferences, focusState, requiredConditions: variant.requiredConditions, verifiedConditions, underlay, ...(sessionRecordQuery ? { sessionRecordQuery } : {}) }, ...(referencePremiseMismatch ? { referencePremiseMismatch } : {}), reason: referencePremiseMismatch ? 'Historical target-review reference requires manual-enable Off; actual automatic preview is captured as a premise mismatch pending independent replacement reference review.' : conditionsMatch && geometryMatches ? 'Exact independent design-to-React pixel comparison.' : 'Real client geometry or scroll region differs from the declared independent design; no masks/resizing/threshold relaxation.' });
  }
  const [main, ...variants] = results;
  return { ...main, variants, status: results.every(row => row.status === 'passed') ? 'passed' : 'failed', catalogue: catalogue.file, catalogueSha256: catalogue.sha256, proposalPreview: catalogue.proposalPreview };
}
