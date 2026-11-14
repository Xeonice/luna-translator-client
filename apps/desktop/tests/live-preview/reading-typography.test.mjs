import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs/promises';
import ts from 'typescript';

// Use the project's compiler, not a newer Node-only TypeScript runtime mode.
const source = await fs.readFile(new URL('../../src/features/reading/reading-typography.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { mainReadingTypography } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

const { manualTranslationFontPx } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('Main uses its approved hierarchy while retaining the HUD preference values', () => {
  const preferences = Object.freeze({ sourceFontPt: 18, translationFontPt: 24 });
  assert.deepEqual(mainReadingTypography(preferences), { sourceFontPx: 23, translationFontPx: 18 });
  assert.deepEqual(preferences, { sourceFontPt: 18, translationFontPt: 24 });
});

test('independent custom preferences remain independent and are not rewritten', () => {
  const preferences = Object.freeze({ sourceFontPt: 29, translationFontPt: 35 });
  const custom = mainReadingTypography(preferences);
  assert.ok(custom.sourceFontPx > mainReadingTypography({ ...preferences, sourceFontPt: 18 }).sourceFontPx);
  assert.equal(custom.translationFontPx, mainReadingTypography({ ...preferences, sourceFontPt: 18 }).translationFontPx);
  assert.equal(custom.sourceFontPx, mainReadingTypography({ ...preferences, translationFontPt: 24 }).sourceFontPx);
  assert.deepEqual(preferences, { sourceFontPt: 29, translationFontPt: 35 });
});

test('finite positive preference boundaries never become an invalid presentation size', () => {
  for (const size of [Number.MIN_VALUE, Number.MIN_VALUE * 2, Number.EPSILON, Number.MAX_VALUE / 2, Number.MAX_VALUE]) {
    const preferences = Object.freeze({ sourceFontPt: size, translationFontPt: size });
    const view = mainReadingTypography(preferences);
    assert.ok(Number.isFinite(view.sourceFontPx) && view.sourceFontPx > 0);
    assert.ok(Number.isFinite(view.translationFontPx) && view.translationFontPx > 0);
    assert.deepEqual(preferences, { sourceFontPt: size, translationFontPt: size });
  }
  assert.equal(mainReadingTypography({ sourceFontPt: Number.MAX_VALUE, translationFontPt: 24 }).sourceFontPx, Number.MAX_VALUE);
});

test('manual results derive their own role without rewriting saved typography', () => {
  const baseline = Object.freeze({ translationFontPt: 24 });
  assert.equal(manualTranslationFontPx(baseline), 19);
  const custom = Object.freeze({ translationFontPt: 35 });
  assert.equal(manualTranslationFontPx(custom), 35 * (19 / 24));
  assert.deepEqual(custom, { translationFontPt: 35 });
  for (const size of [Number.MIN_VALUE, Number.EPSILON, Number.MAX_VALUE]) {
    const view = manualTranslationFontPx(Object.freeze({ translationFontPt: size }));
    assert.ok(Number.isFinite(view) && view > 0);
  }
});
