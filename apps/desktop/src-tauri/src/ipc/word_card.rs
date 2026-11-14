//! Temporary dictionary surface commands; deliberately separate from primary-window ACLs.
use crate::{
    bootstrap::DesktopState,
    ipc::{blocking, role},
    platform::window_manager,
};
use app_core::contracts::*;
use tauri::{AppHandle, Manager, State, WebviewWindow};

#[tauri::command]
pub async fn application_word_card_open(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: OpenWordCardRequest,
) -> AppResult<String> {
    role(&window, &["main", "hud", "learning"])?;
    let coordinator = app
        .state::<std::sync::Arc<window_manager::WindowCoordinator>>()
        .inner()
        .clone();
    let owner_stamp = coordinator.capture_owner(&window)?;
    let sequence = state.word_cards.begin();
    let runtime = state.runtime.clone();
    let unit_id = request.unit_id.clone();
    let revision = request.text_revision;
    let index = request.token_index;
    let object = blocking(move || {
        if revision == 0 {
            return Err(AppError::new(
                "invalid_revision",
                "dictionary",
                "请选择固定文本版本",
            ));
        }
        let unit = runtime.unit(unit_id, revision)?;
        let token = unit
            .tokens
            .get(index as usize)
            .ok_or_else(|| AppError::new("stale_token", "dictionary", "词语位置已失效"))?;
        if token.surface.trim().is_empty() {
            return Err(AppError::new(
                "stale_token",
                "dictionary",
                "请选择原文中的词语",
            ));
        }
        let preferences = runtime.snapshot()?.preferences;
        Ok(FrozenWordCard {
            card_id: String::new(),
            unit,
            token_index: index,
            preferences,
        })
    })
    .await?;
    coordinator.validate_owner(&window, owner_stamp)?;
    state.word_cards.open(
        &app,
        &window,
        sequence,
        object,
        &request.anchor_rect,
        owner_stamp,
    )
}
#[tauri::command]
pub fn application_word_card_snapshot(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> AppResult<FrozenWordCard> {
    let object = state.word_cards.surface(window.label(), None)?.object;
    use crate::platform::runtime_observation::{self, Fact, Id};
    runtime_observation::record((|| {
        Some(Fact::WordCardSnapshot {
            card_id: Id::new(&object.card_id)?,
            unit_id: Id::new(&object.unit.unit_id)?,
            text_revision: object.unit.text_revision,
            token_index: object.token_index,
            source_hash: Id::new(&object.unit.source_hash)?,
        })
    })());
    Ok(object)
}
#[tauri::command]
pub async fn application_word_card_lookup(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    card_id: String,
) -> AppResult<WordLookup> {
    let card = state.word_cards.surface(window.label(), Some(&card_id))?;
    let expected = card
        .object
        .unit
        .tokens
        .get(card.object.token_index as usize)
        .cloned()
        .ok_or_else(|| AppError::new("stale_token", "dictionary", "词语位置已失效"))?;
    let runtime = state.runtime.clone();
    let lookup = blocking(move || {
        runtime.lookup(
            card.object.unit.unit_id,
            card.object.unit.text_revision,
            card.object.token_index,
        )
    })
    .await?;
    if lookup.token != expected {
        return Err(AppError::new(
            "stale_token",
            "dictionary",
            "原句的词法版本已更新，请重新选词",
        ));
    }
    // A late lookup is never authority for a newly selected word.
    state.word_cards.surface(window.label(), Some(&card_id))?;
    Ok(lookup)
}
#[tauri::command]
pub fn application_word_card_close(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    card_id: String,
) -> AppResult<()> {
    state.word_cards.close(&app, window.label(), &card_id)
}
#[tauri::command]
pub async fn application_word_card_action(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopState>,
    card_id: String,
    intent: WordCardIntent,
) -> AppResult<()> {
    state.word_cards.surface(window.label(), Some(&card_id))?;
    let runtime = state.runtime.clone();
    let cards = state.word_cards.clone();
    let label = window.label().to_string();
    let action_card_id = card_id.clone();
    let study_error = blocking(move || {
        cards.with_current(&label, &action_card_id, |card| {
            let current = runtime.unit(
                card.object.unit.unit_id.clone(),
                card.object.unit.text_revision,
            )?;
            if current.source_hash != card.object.unit.source_hash
                || current.tokens.get(card.object.token_index as usize)
                    != card
                        .object
                        .unit
                        .tokens
                        .get(card.object.token_index as usize)
            {
                return Err(AppError::new(
                    "stale_token",
                    "dictionary",
                    "原句的词法版本已更新，请重新选词",
                ));
            }
            runtime.freeze_learning(
                card.object.unit.unit_id.clone(),
                card.object.unit.text_revision,
                Some(card.object.token_index),
            )?;
            let study_error = if intent == WordCardIntent::ContextWord {
                runtime
                    .study(
                        card.object.unit.unit_id.clone(),
                        card.object.unit.text_revision,
                        "context_word".into(),
                        Some(card.object.token_index),
                    )
                    .err()
            } else {
                None
            };
            Ok(study_error)
        })
    })
    .await?;
    state
        .word_cards
        .with_current(window.label(), &card_id, |_| Ok(()))?;
    window_manager::show(&app, WindowRole::Learning)?;
    if let Some(error) = study_error {
        return Err(error);
    }
    state.word_cards.close(&app, window.label(), &card_id)
}

pub fn on_window_event(
    window: &tauri::Window,
    event: &tauri::WindowEvent,
    decision: &window_manager::OwnerGeometryDecision,
) {
    let Some(state) = window.app_handle().try_state::<DesktopState>() else {
        return;
    };
    if window.label().starts_with("word-card-") {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            state
                .word_cards
                .surface_closed(window.app_handle(), window.label());
        }
    } else if let Some(invalidation) = &decision.invalidation {
        state
            .word_cards
            .owner_invalidated(window.app_handle(), invalidation);
    }
}
