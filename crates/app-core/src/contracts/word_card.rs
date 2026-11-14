//! Temporary dictionary surface DTOs, separate from the four primary window roles.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WordCardAnchorRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenWordCardRequest {
    pub unit_id: String,
    pub text_revision: u32,
    pub token_index: u32,
    pub anchor_rect: WordCardAnchorRect,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FrozenWordCard {
    pub card_id: String,
    pub unit: super::ReadingUnit,
    pub token_index: u32,
    pub preferences: super::Preferences,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WordCardIntent {
    Learn,
    ContextWord,
}

pub fn typescript_declarations() -> String {
    [
        WordCardAnchorRect::decl(&ts_rs::Config::default()),
        OpenWordCardRequest::decl(&ts_rs::Config::default()),
        FrozenWordCard::decl(&ts_rs::Config::default()),
        WordCardIntent::decl(&ts_rs::Config::default()),
    ]
    .iter()
    .map(|value| format!("\nexport {value}\n"))
    .collect()
}
