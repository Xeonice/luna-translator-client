//! A bounded, transient dictionary surface. It never becomes a primary WindowRole.
use app_core::contracts::{AppError, AppResult, FrozenWordCard, WordCardAnchorRect};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[cfg(target_os = "macos")]
use tauri::{LogicalPosition, LogicalSize};
#[cfg(not(target_os = "macos"))]
use tauri::{PhysicalPosition, PhysicalSize};

pub const CLOSED_EVENT: &str = "word-card-closed";
#[derive(Clone)]
pub struct ActiveCard {
    generation: u64,
    pub object: FrozenWordCard,
    pub owner: String,
    pub label: String,
    pub owner_stamp: super::window_manager::OwnerGeometryStamp,
}
#[derive(Clone)]
struct CardTicket {
    card_id: String,
    generation: u64,
    owner: String,
    owner_stamp: super::window_manager::OwnerGeometryStamp,
}
impl ActiveCard {
    fn ticket(&self) -> CardTicket {
        CardTicket {
            card_id: self.object.card_id.clone(),
            generation: self.generation,
            owner: self.owner.clone(),
            owner_stamp: self.owner_stamp,
        }
    }
}
#[derive(Default)]
pub struct WordCards {
    sequence: AtomicU64,
    active: Mutex<Option<ActiveCard>>,
    closing: Mutex<std::collections::VecDeque<CardTicket>>,
}
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "dictionary", message)
}
impl WordCards {
    pub fn begin(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn surface(&self, label: &str, card_id: Option<&str>) -> AppResult<ActiveCard> {
        let guard = self
            .active
            .lock()
            .map_err(|_| error("word_card_state", "词卡状态不可用"))?;
        guard
            .as_ref()
            .filter(|card| {
                card.label == label && card_id.is_none_or(|id| id == card.object.card_id)
            })
            .cloned()
            .ok_or_else(|| error("stale_word_card", "词卡已关闭或已选择另一个词"))
    }
    /// Serialize an explicit action against selecting a newer surface. No window work is performed under this lock.
    pub fn with_current<T>(
        &self,
        label: &str,
        card_id: &str,
        action: impl FnOnce(&ActiveCard) -> AppResult<T>,
    ) -> AppResult<T> {
        let active = self
            .active
            .lock()
            .map_err(|_| error("word_card_state", "词卡状态不可用"))?;
        let card = active
            .as_ref()
            .filter(|card| {
                card.label == label
                    && card.object.card_id == card_id
                    && card.generation == self.sequence.load(Ordering::SeqCst)
            })
            .ok_or_else(|| error("stale_word_card", "词卡已关闭或已选择另一个词"))?;
        action(card)
    }
    fn install(&self, sequence: u64, card: ActiveCard) -> AppResult<Option<ActiveCard>> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| error("word_card_state", "词卡状态不可用"))?;
        if sequence != self.sequence.load(Ordering::SeqCst) {
            return Err(error("stale_word_card", "已选择另一个词"));
        }
        Ok(active.replace(card))
    }
    fn take_authorized(&self, requester: &str, card_id: &str) -> AppResult<Option<ActiveCard>> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| error("word_card_state", "词卡状态不可用"))?;
        let Some(card) = active.as_ref() else {
            return Ok(None);
        };
        if card.object.card_id != card_id {
            return Ok(None);
        }
        if card.label != requester && card.owner != requester {
            return Err(error("forbidden_window", "此窗口无权操作该词卡"));
        }
        Ok(active.take())
    }
    pub fn open(
        &self,
        app: &AppHandle,
        owner: &WebviewWindow,
        sequence: u64,
        mut object: FrozenWordCard,
        anchor: &WordCardAnchorRect,
        owner_stamp: super::window_manager::OwnerGeometryStamp,
    ) -> AppResult<String> {
        let origin = owner
            .inner_position()
            .map_err(|_| error("word_card_position", "无法确定原文窗口位置"))?;
        let viewport = owner
            .inner_size()
            .map_err(|_| error("word_card_position", "无法确定原文窗口大小"))?;
        let scale = owner
            .scale_factor()
            .map_err(|_| error("word_card_position", "无法确定窗口缩放"))?;
        let space = if cfg!(target_os = "macos") {
            CoordinateSpace::GlobalLogical
        } else {
            CoordinateSpace::Physical
        };
        // Tao macOS physical origins are per-screen logical origins multiplied by
        // that screen's scale. They cannot be compared across mixed-DPI screens.
        let owner_rect = normalize_native_rect(
            (origin.x, origin.y),
            (viewport.width, viewport.height),
            scale,
            space,
        )?;
        let anchor_scale = scale / space.divisor(scale);
        let point = (
            owner_rect.x + (anchor.left + anchor.width / 2.) * anchor_scale,
            owner_rect.y + anchor.top * anchor_scale,
        );
        let monitor = owner
            .available_monitors()
            .map_err(|_| error("word_card_position", "无法读取屏幕工作区"))?
            .into_iter()
            .find(|monitor| {
                let p = monitor.position();
                let s = monitor.size();
                normalize_native_rect(
                    (p.x, p.y),
                    (s.width, s.height),
                    monitor.scale_factor(),
                    space,
                )
                .is_ok_and(|rect| rect.contains(point))
            })
            .or_else(|| owner.current_monitor().ok().flatten())
            .ok_or_else(|| error("word_card_position", "找不到原文所在屏幕"))?;
        let area = monitor.work_area();
        let surface_scale = monitor.scale_factor() / space.divisor(monitor.scale_factor());
        let area = normalize_native_rect(
            (area.position.x, area.position.y),
            (area.size.width, area.size.height),
            monitor.scale_factor(),
            space,
        )?;
        let placement = place(
            anchor,
            (owner_rect.x, owner_rect.y),
            (owner_rect.width, owner_rect.height),
            anchor_scale,
            surface_scale,
            (area.x, area.y, area.width, area.height),
        )?;
        let card_id = format!("word-card-{sequence}");
        object.card_id = card_id.clone();
        let coordinator = app
            .state::<Arc<super::window_manager::WindowCoordinator>>()
            .inner()
            .clone();
        coordinator.validate_owner(owner, owner_stamp)?;
        let old = self.install(
            sequence,
            ActiveCard {
                generation: sequence,
                object,
                owner: owner.label().into(),
                label: card_id.clone(),
                owner_stamp,
            },
        )?;
        if let Some(old) = old {
            dispose(app, old);
        }
        let builder = WebviewWindowBuilder::new(
            app,
            &card_id,
            WebviewUrl::App("index.html?surface=word-card".into()),
        )
        .title("Luna · 本地词典")
        .position(placement.x / surface_scale, placement.y / surface_scale)
        .inner_size(
            placement.width / surface_scale,
            placement.height / surface_scale,
        )
        .decorations(false)
        .resizable(false)
        .transparent(true)
        .shadow(true)
        .skip_taskbar(true)
        .visible(true)
        .focused(false)
        .focusable(true)
        .accept_first_mouse(true)
        .theme(Some(tauri::Theme::Dark))
        .parent(owner);
        let mut builder = match builder {
            Ok(builder) => builder,
            Err(_) => {
                self.close(app, owner.label(), &card_id)?;
                return Err(error("word_card_window", "无法建立词卡附属窗口"));
            }
        };
        if owner.label() == "hud" {
            builder = builder.always_on_top(true);
        }
        let surface = match builder.build() {
            Ok(surface) => surface,
            Err(_) => {
                self.close(app, owner.label(), &card_id)?;
                return Err(error("word_card_window", "无法打开本地词卡"));
            }
        };
        #[cfg(target_os = "macos")]
        let positioned = surface
            .set_position(LogicalPosition::new(placement.x, placement.y))
            .and_then(|_| surface.set_size(LogicalSize::new(placement.width, placement.height)));
        #[cfg(not(target_os = "macos"))]
        let positioned = surface
            .set_position(PhysicalPosition::new(
                placement.x.round() as i32,
                placement.y.round() as i32,
            ))
            .and_then(|_| {
                surface.set_size(PhysicalSize::new(
                    placement.width.floor() as u32,
                    placement.height.floor() as u32,
                ))
            });
        if positioned.is_err() {
            self.close(app, owner.label(), &card_id)?;
            return Err(error("word_card_position", "无法放置本地词卡"));
        }

        if self.surface(&card_id, Some(&card_id)).is_err() {
            let _ = surface.destroy();
            return Err(error("stale_word_card", "词卡已关闭或已选择另一个词"));
        }
        // macOS tao's show() uses makeKeyAndOrderFront even when the builder was
        // focused(false). The visible(true), focused(false) creation path uses
        // orderFront; do not call show()/set_focus() here.
        #[cfg(target_os = "macos")]
        if surface
            .set_effects(
                tauri::window::EffectsBuilder::new()
                    .effect(tauri::window::Effect::HudWindow)
                    .state(tauri::window::EffectState::Active)
                    .radius(13.)
                    .build(),
            )
            .is_err()
        {
            self.close(app, owner.label(), &card_id)?;
            return Err(error("word_card_window", "无法设置词卡显示材质"));
        }
        // Owner intent may have changed while native construction/positioning
        // crossed the main thread. Installation is already visible to native
        // invalidation, and this final check closes the pre-install race.
        let ticket = CardTicket {
            card_id: card_id.clone(),
            generation: sequence,
            owner: owner.label().into(),
            owner_stamp,
        };
        if let Err(rejection) =
            self.finish_open(&ticket, || coordinator.validate_owner(owner, owner_stamp))
        {
            let (error, card) = *rejection;
            if let Some(card) = card {
                dispose(app, card);
            } else {
                let _ = super::window_manager::defer_owner_effect(move || {
                    let _ = surface.destroy();
                });
            }
            return Err(error);
        }
        trace_card(
            app,
            "card_opened",
            "fixed_word_selected",
            &CardTicket {
                card_id: card_id.clone(),
                generation: sequence,
                owner: owner.label().into(),
                owner_stamp,
            },
        );
        Ok(card_id)
    }
    pub fn close(&self, app: &AppHandle, requester: &str, card_id: &str) -> AppResult<()> {
        let card = self.take_authorized(requester, card_id)?;
        if let Some(card) = card {
            dispose(app, card);
        }
        Ok(())
    }
    fn take_ticket(&self, ticket: &CardTicket) -> Option<ActiveCard> {
        let mut active = self.active.lock().ok()?;
        if active.as_ref().is_some_and(|card| {
            card.object.card_id == ticket.card_id
                && card.generation == ticket.generation
                && card.owner == ticket.owner
                && card.owner_stamp.epoch == ticket.owner_stamp.epoch
                && card.owner_stamp.user_generation == ticket.owner_stamp.user_generation
        }) {
            active.take()
        } else {
            None
        }
    }
    /// Native validation is deliberately outside the card mutex. If owner
    /// intent changed between the pre-install check and construction, remove
    /// only this installed generation. A replacement installed by a reentrant
    /// callback is never cleanup authority for the previous open operation.
    fn finish_open(
        &self,
        ticket: &CardTicket,
        validate: impl FnOnce() -> AppResult<()>,
    ) -> Result<(), Box<(AppError, Option<ActiveCard>)>> {
        if let Err(error) = validate() {
            return Err(Box::new((error, self.take_ticket(ticket))));
        }
        let current = self
            .active
            .lock()
            .map(|active| {
                active.as_ref().is_some_and(|card| {
                    card.object.card_id == ticket.card_id
                        && card.generation == ticket.generation
                        && card.generation == self.sequence.load(Ordering::SeqCst)
                        && card.owner_stamp.epoch == ticket.owner_stamp.epoch
                        && card.owner_stamp.user_generation == ticket.owner_stamp.user_generation
                })
            })
            .unwrap_or(false);
        if current {
            Ok(())
        } else {
            Err(Box::new((
                error("stale_word_card", "词卡已关闭或已选择另一个词"),
                self.take_ticket(ticket),
            )))
        }
    }
    pub fn owner_invalidated(
        self: &Arc<Self>,
        app: &AppHandle,
        invalidation: &super::window_manager::OwnerInvalidation,
    ) {
        let app = app.clone();
        let invalidation = invalidation.clone();
        // Existing explicit learning actions can hold active across SQL. Only
        // carry the immutable owner boundary out of AppKit's notification; all
        // card-lock acquisition runs on the blocking worker, never the main loop.
        let reason = invalidation.reason;
        self.queue_invalidated(invalidation, move |card| {
            trace_card(&app, "card_invalidation_queued", reason, &card.ticket());
            let _ =
                super::window_manager::defer_owner_effect(move || dispose_now(&app, card, reason));
        });
    }
    fn queue_invalidated(
        self: &Arc<Self>,
        invalidation: super::window_manager::OwnerInvalidation,
        effect: impl FnOnce(ActiveCard) + Send + 'static,
    ) {
        let cards = self.clone();
        std::mem::drop(tauri::async_runtime::spawn_blocking(move || {
            if let Some(card) = cards.take_invalidated(&invalidation) {
                effect(card);
            }
        }));
    }
    fn take_invalidated(
        &self,
        invalidation: &super::window_manager::OwnerInvalidation,
    ) -> Option<ActiveCard> {
        let mut active = self.active.lock().ok()?;
        if active.as_ref().is_some_and(|card| {
            card.owner == invalidation.owner
                && card.owner_stamp.epoch == invalidation.epoch
                && card.owner_stamp.user_generation <= invalidation.through_generation
        }) {
            active.take()
        } else {
            None
        }
    }
    pub fn surface_closed(self: &Arc<Self>, app: &AppHandle, label: &str) {
        let cards = self.clone();
        let app = app.clone();
        let label = label.to_owned();
        tauri::async_runtime::spawn_blocking(move || cards.finish_surface_closed(&app, &label));
    }
    fn finish_surface_closed(&self, app: &AppHandle, label: &str) {
        let card = self.active.lock().ok().and_then(|mut active| {
            if active.as_ref().is_some_and(|card| card.label == label) {
                active.take()
            } else {
                None
            }
        });
        if let Some(card) = card {
            trace_card(
                app,
                "native_card_destroyed",
                "surface_destroyed",
                &card.ticket(),
            );
            let _ = app.emit_to(card.owner, CLOSED_EVENT, card.object.card_id);
        }
        let closed = self.closing.lock().ok().and_then(|mut closing| {
            closing
                .iter()
                .position(|ticket| ticket.card_id == label)
                .and_then(|index| closing.remove(index))
        });
        if let Some(ticket) = closed {
            trace_card(app, "native_card_destroyed", "surface_destroyed", &ticket);
        }
    }
}
fn dispose(app: &AppHandle, card: ActiveCard) {
    let app = app.clone();
    let _ = super::window_manager::defer_owner_effect(move || {
        dispose_now(&app, card, "explicit_close_or_replacement")
    });
}
fn trace_card(app: &AppHandle, phase: &'static str, reason: &'static str, ticket: &CardTicket) {
    if let Some(coordinator) = app.try_state::<Arc<super::window_manager::WindowCoordinator>>() {
        coordinator.card_fact(
            phase,
            reason,
            &ticket.owner,
            ticket.owner_stamp,
            &ticket.card_id,
            ticket.generation,
        );
    }
}
fn dispose_now(app: &AppHandle, card: ActiveCard, reason: &'static str) {
    let ticket = card.ticket();
    trace_card(app, "card_close_requested", reason, &ticket);
    if let Some(window) = app.get_webview_window(&card.label) {
        if let Some(state) = app.try_state::<crate::bootstrap::DesktopState>()
            && let Ok(mut closing) = state.word_cards.closing.lock()
        {
            if closing.len() == 32 {
                closing.pop_front();
            }
            closing.push_back(ticket.clone());
        }
        let result = window.destroy();
        trace_card(
            app,
            "card_destroy_dispatched",
            if result.is_ok() {
                "native_destroy_accepted"
            } else {
                "native_destroy_failed"
            },
            &ticket,
        );
    }
    let _ = app.emit_to(card.owner, CLOSED_EVENT, card.object.card_id);
}
#[derive(Clone, Copy)]
enum CoordinateSpace {
    GlobalLogical,
    Physical,
}
impl CoordinateSpace {
    fn divisor(self, scale: f64) -> f64 {
        match self {
            Self::GlobalLogical => scale,
            Self::Physical => 1.,
        }
    }
}
#[derive(Debug, PartialEq)]
struct GeometryRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl GeometryRect {
    fn contains(&self, point: (f64, f64)) -> bool {
        point.0 >= self.x
            && point.0 < self.x + self.width
            && point.1 >= self.y
            && point.1 < self.y + self.height
    }
}
fn normalize_native_rect(
    origin: (i32, i32),
    size: (u32, u32),
    scale: f64,
    space: CoordinateSpace,
) -> AppResult<GeometryRect> {
    if !scale.is_finite() || scale <= 0. {
        return Err(error("word_card_position", "屏幕缩放无效"));
    }
    let divisor = space.divisor(scale);
    Ok(GeometryRect {
        x: f64::from(origin.0) / divisor,
        y: f64::from(origin.1) / divisor,
        width: f64::from(size.0) / divisor,
        height: f64::from(size.1) / divisor,
    })
}
#[derive(Debug, PartialEq)]
struct Placement {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn place(
    anchor: &WordCardAnchorRect,
    origin: (f64, f64),
    viewport: (f64, f64),
    scale: f64,
    surface_scale: f64,
    area: (f64, f64, f64, f64),
) -> AppResult<Placement> {
    let invalid = || error("word_card_anchor", "选词位置已失效，请重新选择可见词语");
    if !surface_scale.is_finite()
        || surface_scale <= 0.
        || !scale.is_finite()
        || scale <= 0.
        || ![
            anchor.left,
            anchor.top,
            anchor.width,
            anchor.height,
            origin.0,
            origin.1,
            viewport.0,
            viewport.1,
            area.0,
            area.1,
            area.2,
            area.3,
        ]
        .iter()
        .all(|v| v.is_finite())
        || anchor.width <= 0.
        || anchor.height <= 0.
        || anchor.left < 0.
        || anchor.top < 0.
        || (anchor.left + anchor.width) * scale > viewport.0 + 1.
        || (anchor.top + anchor.height) * scale > viewport.1 + 1.
    {
        return Err(invalid());
    }
    let margin = 8. * surface_scale;
    let width = (360. * surface_scale).min(area.2 - 2. * margin);
    let mut height = (460. * surface_scale).min(area.3 - 2. * margin);
    if width < 160. * surface_scale || height < 120. * surface_scale {
        return Err(invalid());
    }
    let min_x = area.0 + margin;
    let max_x = area.0 + area.2 - margin - width;
    let min_y = area.1 + margin;
    let bottom = area.1 + area.3 - margin;
    let anchor_x = origin.0 + anchor.left * scale;
    let anchor_top = origin.1 + anchor.top * scale;
    let above = anchor_top - margin - height;
    let y = if above >= min_y {
        above
    } else {
        let below = anchor_top + anchor.height * scale + margin;
        let available = bottom - below;
        if available >= 120. * surface_scale {
            height = height.min(available);
            below
        } else {
            height = height.min((anchor_top - margin - min_y).max(120. * surface_scale));
            anchor_top - margin - height
        }
    };
    Ok(Placement {
        x: anchor_x.clamp(min_x, max_x),
        y: y.clamp(min_y, bottom - height),
        width,
        height,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn anchor(left: f64, top: f64) -> WordCardAnchorRect {
        WordCardAnchorRect {
            left,
            top,
            width: 48.,
            height: 24.,
        }
    }
    fn card(id: &str, source: &str) -> ActiveCard {
        ActiveCard {
            generation: id.rsplit('-').next().unwrap().parse().unwrap(),
            owner: "hud".into(),
            label: id.into(),
            owner_stamp: super::super::window_manager::OwnerGeometryStamp {
                epoch: 1,
                user_generation: 0,
                revision: 0,
                actual: super::super::window_manager::PhysicalGeometry {
                    epoch: 1,
                    x: 0.,
                    y: 100.,
                    width: 660,
                    height: 280,
                    scale: 1.,
                    live_resize: 0,
                },
            },
            object: FrozenWordCard {
                card_id: id.into(),
                unit: app_core::contracts::ReadingUnit {
                    source_summary: None,
                    unit_id: "unit".into(),
                    session_id: "session".into(),
                    game_id: None,
                    text_revision: 1,
                    source_text: source.into(),
                    source_hash: "hash".into(),
                    source_language: "ja".into(),
                    created_at_ms: 0.,
                    translations: vec![],
                    tokens: vec![],
                },
                token_index: 0,
                preferences: Default::default(),
            },
        }
    }
    #[test]
    fn a_late_open_cannot_replace_the_new_selection_and_the_object_is_independent() {
        let state = WordCards::default();
        let a = state.begin();
        let b = state.begin();
        let mut original = card("word-card-2", "図書館");
        state.install(b, original.clone()).unwrap();
        original.object.unit.source_text = "新しい台詞".into();
        assert!(state.install(a, card("word-card-1", "雨")).is_err());
        assert_eq!(
            state
                .surface("word-card-2", Some("word-card-2"))
                .unwrap()
                .object
                .unit
                .source_text,
            "図書館"
        );
        assert!(state.surface("word-card-1", Some("word-card-1")).is_err());
    }
    #[test]
    fn deferred_old_geometry_ticket_cannot_remove_a_new_card_or_owner_instance() {
        let state = WordCards::default();
        let first = state.begin();
        state.install(first, card("word-card-1", "言葉")).unwrap();
        let old = state.surface("word-card-1", None).unwrap().ticket();
        let second = state.begin();
        state
            .install(second, card("word-card-2", "感じる"))
            .unwrap();
        assert!(state.take_ticket(&old).is_none());
        let current = state.surface("word-card-2", None).unwrap().ticket();
        let mut wrong_epoch = current.clone();
        wrong_epoch.owner_stamp.epoch += 1;
        assert!(state.take_ticket(&wrong_epoch).is_none());
        let mut wrong_generation = current.clone();
        wrong_generation.owner_stamp.user_generation += 1;
        assert!(state.take_ticket(&wrong_generation).is_none());
        assert!(state.take_ticket(&current).is_some());
        assert!(state.take_ticket(&current).is_none());
    }
    #[test]
    fn owner_change_between_validation_and_install_is_rejected_after_native_construction() {
        let state = WordCards::default();
        let sequence = state.begin();
        let opening = card("word-card-1", "言葉");
        let ticket = opening.ticket();
        // The first owner validation succeeds. A native user edge then occurs
        // before install, when there is no active card for that edge to close.
        let owner_generation = AtomicU64::new(ticket.owner_stamp.user_generation);
        let validate = || {
            if owner_generation.load(Ordering::SeqCst) != ticket.owner_stamp.user_generation {
                Err(error("word_card_owner_changed", "owner changed"))
            } else {
                Ok(())
            }
        };
        assert!(validate().is_ok());
        owner_generation.fetch_add(1, Ordering::SeqCst);
        state.install(sequence, opening).unwrap();
        let result = state.finish_open(&ticket, validate);
        let (error, removed) = *result.err().unwrap();
        assert_eq!(error.code, "word_card_owner_changed");
        assert_eq!(removed.unwrap().object.card_id, "word-card-1");
        assert!(state.surface("word-card-1", None).is_err());
    }
    #[test]
    fn reentrant_owner_validation_cannot_remove_a_replacement_and_holds_no_card_lock() {
        let state = WordCards::default();
        let first = state.begin();
        let original = card("word-card-1", "言葉");
        let ticket = original.ticket();
        state.install(first, original).unwrap();
        let result = state.finish_open(&ticket, || {
            let second = state.begin();
            state.install(second, card("word-card-2", "感じる"))?;
            Err(error("word_card_owner_changed", "native user edge"))
        });
        assert!(result.err().unwrap().1.is_none());
        assert_eq!(
            state
                .surface("word-card-2", None)
                .unwrap()
                .object
                .unit
                .source_text,
            "感じる"
        );
    }
    #[test]
    fn delayed_owner_boundary_only_takes_its_old_instance_and_user_generation() {
        let state = WordCards::default();
        let first = state.begin();
        state.install(first, card("word-card-1", "言葉")).unwrap();
        let boundary = super::super::window_manager::OwnerInvalidation {
            owner: "hud".into(),
            epoch: 1,
            through_generation: 0,
            reason: "user_live_resize",
        };
        assert_eq!(
            state.take_invalidated(&boundary).unwrap().object.card_id,
            "word-card-1"
        );
        let second = state.begin();
        let mut new = card("word-card-2", "感じる");
        new.owner_stamp.user_generation = 1;
        state.install(second, new).unwrap();
        assert!(state.take_invalidated(&boundary).is_none());
        let third = state.begin();
        let mut new = card("word-card-3", "図書館");
        new.owner_stamp.epoch = 2;
        state.install(third, new).unwrap();
        assert!(state.take_invalidated(&boundary).is_none());
    }
    #[test]
    fn queued_native_invalidation_does_not_wait_for_an_explicit_action_holding_the_card_lock() {
        let cards = Arc::new(WordCards::default());
        let sequence = cards.begin();
        cards
            .install(sequence, card("word-card-1", "言葉"))
            .unwrap();
        let (entered, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let action_cards = cards.clone();
        let action = std::thread::spawn(move || {
            action_cards.with_current("word-card-1", "word-card-1", |_| {
                entered.send(()).unwrap();
                wait.recv().unwrap();
                Ok(())
            })
        });
        ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let (returned, observe_return) = std::sync::mpsc::channel();
        let (effect, observe_effect) = std::sync::mpsc::channel();
        let callback_cards = cards.clone();
        let callback = std::thread::spawn(move || {
            callback_cards.queue_invalidated(
                super::super::window_manager::OwnerInvalidation {
                    owner: "hud".into(),
                    epoch: 1,
                    through_generation: 0,
                    reason: "user_live_resize",
                },
                move |card| {
                    effect.send(card.object.card_id).unwrap();
                },
            );
            returned.send(()).unwrap();
        });
        let nonblocking = observe_return.recv_timeout(std::time::Duration::from_secs(2));
        assert!(observe_effect.try_recv().is_err());
        release.send(()).unwrap();
        action.join().unwrap().unwrap();
        callback.join().unwrap();
        assert!(
            nonblocking.is_ok(),
            "native notification path waited for the action/card lock"
        );
        assert_eq!(
            observe_effect
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            "word-card-1"
        );
    }
    #[test]
    fn a_queued_action_is_rejected_when_a_newer_selection_has_started() {
        let state = WordCards::default();
        let first = state.begin();
        state.install(first, card("word-card-1", "駅")).unwrap();
        let _newer_request = state.begin();
        let mut called = false;
        assert!(
            state
                .with_current("word-card-1", "word-card-1", |_| {
                    called = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!called);
    }
    #[test]
    fn temporary_surface_identity_is_not_a_primary_window_or_a_prefix_capability() {
        let state = WordCards::default();
        let sequence = state.begin();
        state.install(sequence, card("word-card-1", "駅")).unwrap();
        for label in [
            "hud",
            "main",
            "settings",
            "learning",
            "word-card-999",
            "word-card-",
        ] {
            assert!(state.surface(label, None).is_err());
        }
        assert!(state.surface("word-card-1", Some("wrong-card")).is_err());
        assert!(state.take_authorized("settings", "word-card-1").is_err());
        assert!(
            state
                .take_authorized("word-card-999", "word-card-1")
                .is_err()
        );
        assert!(
            state
                .take_authorized("hud", "older-card")
                .unwrap()
                .is_none()
        );
        assert!(state.surface("word-card-1", None).is_ok());
        assert!(
            state
                .take_authorized("word-card-1", "word-card-1")
                .unwrap()
                .is_some()
        );
        assert!(state.surface("word-card-1", None).is_err());
    }
    #[test]
    fn macos_mixed_dpi_monitor_selection_uses_one_global_logical_space() {
        let physical_retina =
            normalize_native_rect((0, 0), (2880, 1800), 2., CoordinateSpace::Physical).unwrap();
        let physical_external =
            normalize_native_rect((1440, 0), (1920, 1080), 1., CoordinateSpace::Physical).unwrap();
        let owner =
            normalize_native_rect((1500, 200), (660, 280), 1., CoordinateSpace::GlobalLogical)
                .unwrap();
        let point = (owner.x + 40. + 48. / 2., owner.y + 40.);
        // This is the real Tao macOS ambiguity: both per-monitor physical rects overlap.
        assert!(physical_retina.contains(point) && physical_external.contains(point));
        let retina =
            normalize_native_rect((0, 0), (2880, 1800), 2., CoordinateSpace::GlobalLogical)
                .unwrap();
        let external =
            normalize_native_rect((1440, 0), (1920, 1080), 1., CoordinateSpace::GlobalLogical)
                .unwrap();
        assert!(!retina.contains(point));
        assert!(external.contains(point));
        assert_eq!(
            [retina, external]
                .iter()
                .position(|rect| rect.contains(point)),
            Some(1)
        );
        let work_area =
            normalize_native_rect((1440, 25), (1920, 1055), 1., CoordinateSpace::GlobalLogical)
                .unwrap();
        let placement = place(
            &anchor(40., 40.),
            (owner.x, owner.y),
            (owner.width, owner.height),
            1.,
            1.,
            (work_area.x, work_area.y, work_area.width, work_area.height),
        )
        .unwrap();
        assert_eq!(
            placement,
            Placement {
                x: 1540.,
                y: 272.,
                width: 360.,
                height: 460.
            }
        );
    }
    #[test]
    fn macos_same_dpi_monitors_and_retina_owner_preserve_logical_origin_and_work_area() {
        let primary =
            normalize_native_rect((0, 0), (2880, 1800), 2., CoordinateSpace::GlobalLogical)
                .unwrap();
        let right =
            normalize_native_rect((2880, 0), (3840, 2160), 2., CoordinateSpace::GlobalLogical)
                .unwrap();
        let owner =
            normalize_native_rect((3200, 800), (1320, 560), 2., CoordinateSpace::GlobalLogical)
                .unwrap();
        let point = (owner.x + 64., owner.y + 40.);
        assert!(!primary.contains(point));
        assert!(right.contains(point));
        assert_eq!(
            (owner.x, owner.y, owner.width, owner.height),
            (1600., 400., 660., 280.)
        );
        let area = normalize_native_rect((0, 50), (2880, 1750), 2., CoordinateSpace::GlobalLogical)
            .unwrap();
        assert_eq!(
            area,
            GeometryRect {
                x: 0.,
                y: 25.,
                width: 1440.,
                height: 875.
            }
        );
        assert!(normalize_native_rect((0, 0), (1, 1), 0., CoordinateSpace::GlobalLogical).is_err());
    }
    #[test]
    fn windows_monitor_rectangles_keep_native_physical_coordinates() {
        let main =
            normalize_native_rect((0, 0), (2880, 1800), 2., CoordinateSpace::Physical).unwrap();
        let right =
            normalize_native_rect((2880, 0), (1920, 1080), 1., CoordinateSpace::Physical).unwrap();
        assert_eq!(main.width, 2880.);
        assert!(!main.contains((3064., 240.)));
        assert!(right.contains((3064., 240.)));
        let owner =
            normalize_native_rect((3000, 200), (660, 280), 1., CoordinateSpace::Physical).unwrap();
        let placement = place(
            &anchor(40., 40.),
            (owner.x, owner.y),
            (owner.width, owner.height),
            1.,
            1.,
            (right.x, right.y, right.width, right.height),
        )
        .unwrap();
        assert_eq!(
            placement,
            Placement {
                x: 3040.,
                y: 272.,
                width: 360.,
                height: 460.
            }
        );
    }
    #[test]
    fn content_origin_and_scale_place_above_beyond_the_hud() {
        let p = place(
            &anchor(40., 40.),
            (200., 700.),
            (1320., 560.),
            2.,
            2.,
            (0., 0., 2560., 1600.),
        )
        .unwrap();
        assert_eq!(
            p,
            Placement {
                x: 280.,
                y: 844.,
                width: 720.,
                height: 740.
            }
        );
        let p = place(
            &anchor(40., 40.),
            (200., 700.),
            (660., 280.),
            1.,
            1.,
            (0., 0., 1920., 1080.),
        )
        .unwrap();
        assert_eq!(p.y, 272.);
        assert!(p.y < 700.);
        assert_eq!(p.x, 240.);
    }
    #[test]
    fn top_edge_flips_down_and_negative_monitor_edges_are_clamped() {
        let p = place(
            &anchor(600., 5.),
            (-700., 20.),
            (660., 280.),
            1.,
            1.,
            (-1440., 0., 1440., 900.),
        )
        .unwrap();
        assert_eq!(p.x, -368.);
        assert_eq!(p.y, 57.);
        let p = place(
            &anchor(5., 240.),
            (-1450., 680.),
            (660., 280.),
            1.,
            1.,
            (-1440., 0., 1440., 900.),
        )
        .unwrap();
        assert_eq!(p.x, -1432.);
        assert_eq!(p.y, 432.);
    }
    #[test]
    fn card_size_uses_destination_screen_scale_while_anchor_uses_owner_scale() {
        let p = place(
            &anchor(40., 40.),
            (200., 700.),
            (1320., 560.),
            2.,
            1.,
            (0., 0., 2560., 1600.),
        )
        .unwrap();
        assert_eq!(
            p,
            Placement {
                x: 280.,
                y: 312.,
                width: 360.,
                height: 460.
            }
        );
    }
    #[test]
    fn malformed_or_off_viewport_dom_anchors_are_rejected() {
        for rect in [
            anchor(-1., 0.),
            anchor(630., 0.),
            anchor(1., 270.),
            anchor(f64::NAN, 0.),
            WordCardAnchorRect {
                width: 0.,
                ..anchor(1., 1.)
            },
        ] {
            assert!(
                place(
                    &rect,
                    (0., 0.),
                    (660., 280.),
                    1.,
                    1.,
                    (0., 0., 1920., 1080.)
                )
                .is_err()
            );
        }
        assert!(
            place(
                &anchor(1., 1.),
                (0., 0.),
                (660., 280.),
                0.,
                1.,
                (0., 0., 1920., 1080.)
            )
            .is_err()
        );
    }
}
