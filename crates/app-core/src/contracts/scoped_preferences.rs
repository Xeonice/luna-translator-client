//! Saved scope values and effective inheritance are separate facts.
use super::Preferences;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreferenceField {
    Theme,
    TransparencyMode,
    ShowRuby,
    ShowPos,
    SourceFontPt,
    TranslationFontPt,
    HudWidthPt,
    Shortcuts,
    SegmentationEnabled,
    ReadingDisplayMode,
    HudBackgroundOpacity,
    HudCompact,
    HudAlwaysOnTop,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopedPreferences {
    pub scope_key: String,
    pub saved: Option<Preferences>,
    pub effective: Preferences,
    pub global: Preferences,
    pub head_revision: Option<u32>,
    pub preference_revision_id: Option<String>,
    pub overridden_fields: Vec<PreferenceField>,
    pub inherited_draft: Option<Preferences>,
}
