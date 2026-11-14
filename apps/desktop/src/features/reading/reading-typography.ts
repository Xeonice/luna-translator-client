import type { Preferences } from '../../bridge/contracts.generated';

function scale(value: number, ratio: number) {
  const derived = value * ratio;
  // Persisted values are validated by Rust. Rendering must keep them finite
  // and positive even at the representable limits.
  return Number.isFinite(derived) && derived > 0 ? derived : value;
}

/**
 * The approved Main reading surface uses 23/18 at the HUD's 18/24 baseline.
 * These are presentation ratios, not replacement preferences or pt conversions.
 * Keep independent user adjustments and the HUD's original values intact.
 */
export function mainReadingTypography(preferences: Pick<Preferences, 'sourceFontPt' | 'translationFontPt'>) {
  return {
    sourceFontPx: scale(preferences.sourceFontPt, 23 / 18),
    translationFontPx: scale(preferences.translationFontPt, 18 / 24),
  };
}

/** The approved manual result has its own 19px role at the same 24px baseline. */
export function manualTranslationFontPx(preferences: Pick<Preferences, 'translationFontPt'>) {
  return scale(preferences.translationFontPt, 19 / 24);
}
