//! Playfield widget — owns ALL pointer input on the game board, hit-tests
//! piles directly, drives the drag-and-drop interaction, and emits
//! `Move`s into the active `DynGameSession`.
//!
//! Per CLAUDE.md "Drag is owned by GameWidget": pile-widgets do not
//! exist. `paint_pile` is a free function called from this widget's
//! `paint`.

mod animations;
mod banners;
mod celebrate;
mod drag;
mod hints;
mod pile_click;

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::render::{paint_pile, CardSpriteAtlas};

use super::animation::{animated_deck_faces, CardAnim, DeckFace, DeckFlipAnim};
use super::app_model::{Screen, SharedModel};
use super::layout;

use agg_gui::confetti::ConfettiSystem;
use web_time::Instant;

use celebrate::{CelebrationAction, WinLatch};
use drag::DragState;
use hints::{HintAnim, HintFadeOut, HintPulse};

// Win-celebration burst parameters live next to the other render color
// consts in `render`.
use crate::render::{CONFETTI_COUNT, CONFETTI_PALETTE};

/// Chrome-aware playfield rect for the current viewport.
fn playfield_rect(bounds: Rect) -> Rect {
    layout::compute(Size::new(bounds.width, bounds.height)).playfield_rect
}

/// Maximum pointer drift between mouse-down/up for click-to-move.
const CLICK_MOVE_RADIUS: f64 = 6.0;

pub struct GameWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    atlas: CardSpriteAtlas,
    drag: Option<DragState>,
    /// Last MouseDown timestamp + virtual-coord position; used to
    /// detect double-clicks for the auto-foundation shortcut.
    last_click: Option<(web_time::Instant, f64, f64)>,
    /// In-flight card animations (stock dispense, etc.). Painted on
    /// top of static piles each frame; cleared as entries complete.
    animations: Vec<CardAnim>,
    /// In-flight DECK animations — currently driving Klondike's
    /// waste→stock recycle, where the entire waste pile flips back
    /// onto the stock as one 3-D thick box.
    deck_animations: Vec<DeckFlipAnim>,
    /// Current Spider Hint ghost-preview animation, if any. Lives
    /// alongside the static yellow rect highlight that
    /// `paint_spider_hint_overlay` draws — the rects say "here are the
    /// source / destination" and the ghost animates the move once.
    hint_anim: Option<HintAnim>,
    /// Last `AppModel::spider_hint_seq` value this widget noticed. Used
    /// to detect a fresh Hint-button press (including re-clicks with
    /// the same recommended move) so we can re-trigger the preview.
    last_hint_seq: u64,
    /// One-shot fade-up-then-down pulse on the hint rect — kicked off
    /// every time the Hint button fires so the player's eye snaps to
    /// the highlight even when the static rect alone is too quiet
    /// (e.g. the StockDeal recommendation, which has no ghost
    /// preview to back it up).
    hint_pulse: Option<HintPulse>,
    /// Smooth fade-out applied to the hint rect when the user
    /// interacts (mouse-down on the playfield, applies a move, undoes).
    /// Once finished, `displayed_hint` clears.
    hint_fade_out: Option<HintFadeOut>,
    /// The hint the widget is actually painting. Tracks
    /// `model.spider_hint` while the rect is being shown, but stays
    /// non-`None` through the fade-out tail so the rect doesn't
    /// disappear in a single frame when the model clears the hint.
    displayed_hint: Option<crate::games::spider::SpiderHint>,
    /// Edge detector for the win celebration — fires the confetti
    /// burst once on the not-won -> won transition (see `celebrate`).
    win_latch: WinLatch,
    /// Live win-celebration confetti, if a burst is currently playing.
    /// Painted last (on top of everything) and dropped once every flake
    /// has expired. Paint-only: input keeps working while it plays.
    confetti: Option<ConfettiSystem>,
    /// Wall-clock of the previous confetti tick, for computing the
    /// per-frame `dt` the simulation advances by. `None` on the first
    /// frame of a burst (dt = 0).
    confetti_last_tick: Option<Instant>,
}

impl GameWidget {
    pub fn new(model: SharedModel, font: Arc<Font>, atlas: CardSpriteAtlas) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            atlas,
            drag: None,
            last_click: None,
            animations: Vec::new(),
            deck_animations: Vec::new(),
            hint_anim: None,
            last_hint_seq: 0,
            hint_pulse: None,
            hint_fade_out: None,
            displayed_hint: None,
            win_latch: WinLatch::default(),
            confetti: None,
            confetti_last_tick: None,
        }
    }

    /// Replace the sprite cache when the current card dimensions
    /// change (window resize → game's `pile_layout` returned a
    /// different `card_w/card_h`) or the device pixel ratio changes.
    /// Atlas pixel resolution = `card_w * DPR` so each sprite blits
    /// 1:1 with the physical pixels at draw time.
    fn ensure_atlas_for_session(&mut self) {
        let dpr = agg_gui::device_scale().max(0.5);
        let (card_w, card_h) = self
            .model
            .borrow()
            .session
            .as_ref()
            .and_then(|s| s.piles().iter().next().map(|p| (p.card_w, p.card_h)))
            .unwrap_or((90.0, 126.0));
        let scale_unchanged = (dpr - self.atlas.render_scale).abs() < 0.02;
        let dims_unchanged = (card_w - self.atlas.card_w_logical).abs() < 0.5
            && (card_h - self.atlas.card_h_logical).abs() < 0.5;
        if scale_unchanged && dims_unchanged {
            return;
        }
        self.atlas = CardSpriteAtlas::build(&self.font, card_w, card_h, dpr);
    }
}

impl Widget for GameWidget {
    fn type_name(&self) -> &'static str {
        "GameWidget"
    }
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // Re-layout the active session's piles to the new playfield
        // rect — every pile's origin/size is in SCREEN coordinates and
        // depends on `bounds`, so a window resize that doesn't go
        // through here would leave stale positions.
        let rect = playfield_rect(bounds);
        if let Some(s) = self.model.borrow_mut().session.as_mut() {
            s.relayout(rect);
        }
    }
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }
    fn layout(&mut self, available: Size) -> Size {
        available
    }

    fn is_visible(&self) -> bool {
        let s = self.model.borrow().screen;
        matches!(s, Screen::Game | Screen::Won)
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        self.ensure_atlas_for_session();
        // Drop completed animations BEFORE this frame's paint so a
        // landed card lights up in the same frame the in-flight one
        // would otherwise still draw.
        self.animations.retain(|a| !a.done());
        self.deck_animations.retain(|a| !a.done());
        // Spider Hint: detect a fresh Hint button press and snapshot
        // a new ghost preview before we paint anything.
        self.tick_hint_animation();

        // Paint piles directly — pile origins are already in screen
        // coordinates (set by `set_bounds` → `relayout`).
        let model = self.model.borrow();
        let pf = playfield_rect(self.bounds);
        if let Some(session) = model.session.as_ref() {
            let piles = session.piles();
            for pile in piles.iter() {
                let drag_hide = self
                    .drag
                    .as_ref()
                    .filter(|d| d.source_pile == pile.id)
                    .map(|d| d.start_idx);
                let anim_hide =
                    animations::hide_from_for(&self.animations, &self.deck_animations, pile.id);
                let hide_from = match (drag_hide, anim_hide) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                paint_pile(ctx, pile, hide_from, &self.atlas);
            }
        }
        drop(model);

        // Spider Hint overlay sits above the static piles but under
        // animations/drag so a moving card never gets a stale yellow halo.
        self.paint_spider_hint_overlay(ctx);
        // The one-shot ghost preview rides on top of the static
        // highlight; it fades to 0 alpha and the widget drops it.
        self.paint_hint_animation(ctx);

        // Paint in-flight card animations on top of the static
        // piles. Each animation projects a 3-D Y-axis-rotated card
        // through a short-focal-length perspective and renders the
        // resulting trapezoidal quad via `draw_image_rgba_corners`
        // — a real wgpu textured quad, NOT a 2-D horizontal squash.
        // The face shown swaps at the halfway point so the texture
        // never paints mirrored.
        // Source-reveal backs are the lowest animation layer. They
        // may appear before the flip starts, but must stay under the
        // run that is still visually sitting above them.
        for anim in &self.animations {
            if !animations::is_source_reveal_anim(anim)
                || anim.has_started()
                || !anim.should_paint_now()
            {
                continue;
            }
            animations::paint_card_animation(ctx, &self.atlas, anim);
        }
        // Held-but-not-started transfer cards preserve an intermediate
        // board state while later automatic moves wait their turn.
        // Late-held cards (auto-collapse top cards just landed by the
        // user move) only paint after their `late_appear_at`, so the
        // gating runs through `should_paint_now`.
        for anim in &self.animations {
            if animations::is_source_reveal_anim(anim)
                || anim.has_started()
                || !anim.should_paint_now()
            {
                continue;
            }
            animations::paint_card_animation(ctx, &self.atlas, anim);
        }
        for anim in &self.animations {
            if !anim.has_started() {
                continue;
            }
            animations::paint_card_animation(ctx, &self.atlas, anim);
        }

        // Deck-flip animations: render each in-flight DeckFlipAnim
        // as a 3-D box. Faces paint back-to-front (painter's
        // algorithm) and each face binds the appropriate texture —
        // top face = waste's top card, bottom face = card back, side
        // faces = procedural stripe texture.
        for anim in &self.deck_animations {
            if !anim.has_started() {
                continue;
            }
            for face in animated_deck_faces(anim) {
                match face.face {
                    DeckFace::Top => {
                        let sprite = self.atlas.face(anim.top_card.suit, anim.top_card.rank);
                        ctx.draw_image_rgba_corners(
                            &sprite,
                            self.atlas.px_w,
                            self.atlas.px_h,
                            face.corners,
                        );
                    }
                    DeckFace::Bottom => {
                        let sprite = self.atlas.back();
                        ctx.draw_image_rgba_corners(
                            &sprite,
                            self.atlas.px_w,
                            self.atlas.px_h,
                            face.corners,
                        );
                    }
                    DeckFace::LeftSide | DeckFace::RightSide => {
                        ctx.draw_image_rgba_corners(
                            &anim.side_texture,
                            anim.side_tex_w,
                            anim.side_tex_h,
                            face.corners,
                        );
                    }
                }
            }
        }

        // Paint dragged cards on top.
        if let Some(drag) = self.drag.clone() {
            self.paint_dragged(ctx, &drag);
        }

        // Banners (win / Mom's king-pickup prompt).
        if self.model.borrow().screen == Screen::Won {
            banners::paint_win_banner(ctx, &self.font, pf);
        }
        if self.model.borrow().moms_waiting_king_at.is_some() {
            banners::paint_moms_prompt(ctx, &self.font, pf, "Select a King for the empty slot");
        }

        // Transient status banner (e.g. "No moves" after a fruitless
        // Hint press). `tick_toast` drops it after the lifetime.
        self.model.borrow_mut().tick_toast();
        let toast = self.model.borrow().toast.clone();
        if let Some((msg, started)) = toast {
            // Sit the toast just below the top edge of the playfield so
            // it doesn't collide with the menu bar or the win banner.
            let toast_y = pf.y + pf.height - 80.0;
            super::toast::paint_toast(ctx, &self.font, pf.x, toast_y, pf.width, &msg, started);
        }

        // Win celebration — fire a one-shot confetti burst on the
        // not-won -> won edge, then advance + paint it LAST so the
        // flakes sit on top of the piles, banner, and toast. Confetti
        // is paint-only, so input keeps working while it plays. Any
        // non-won frame (normal play, a new deal / restart / switch, or
        // returning from Home) drops the burst so a stale one can't
        // linger unpainted.
        let won = self.model.borrow().screen == Screen::Won;
        match self.win_latch.observe(won) {
            CelebrationAction::Fire => {
                // Deterministic, wasm32-clean seed derived from the deal
                // seed (no OS randomness). Salted so it doesn't mirror
                // the deal's own RNG stream.
                let seed = self
                    .model
                    .borrow()
                    .session
                    .as_ref()
                    .map(|s| s.seed())
                    .unwrap_or(0)
                    ^ 0x57E2_D0FF_C0FF_EE01;
                self.confetti =
                    Some(ConfettiSystem::burst(pf, CONFETTI_COUNT, CONFETTI_PALETTE, seed));
                self.confetti_last_tick = None;
            }
            CelebrationAction::Drop => {
                self.confetti = None;
                self.confetti_last_tick = None;
            }
            CelebrationAction::Keep => {}
        }
        if let Some(confetti) = self.confetti.as_mut() {
            let now = Instant::now();
            let dt = self
                .confetti_last_tick
                .map(|prev| now.duration_since(prev).as_secs_f64())
                .unwrap_or(0.0);
            self.confetti_last_tick = Some(now);
            if confetti.tick(dt) {
                confetti.paint(ctx);
            } else {
                self.confetti = None;
                self.confetti_last_tick = None;
            }
        }
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        match event {
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                // Coordinates are already in screen space; pile
                // origins live there too after `relayout`.
                let (vx, vy) = (pos.x, pos.y);
                let is_double = self.is_double_click(vx, vy);
                self.last_click = Some((web_time::Instant::now(), vx, vy));
                // Touching the playfield retires whatever hint is on
                // screen — smooth fade-out from the current alpha.
                self.start_hint_fade_out();

                if self.is_moms() {
                    if self.try_moms_click(vx, vy) {
                        return EventResult::Consumed;
                    }
                    return EventResult::Ignored;
                }
                if is_double && self.try_double_click_to_foundation(vx, vy) {
                    self.last_click = None;
                    return EventResult::Consumed;
                }
                if self.try_pile_click(vx, vy) {
                    return EventResult::Consumed;
                }
                if self.try_start_drag(vx, vy) {
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            Event::MouseMove { pos } => {
                if let Some(drag) = self.drag.as_mut() {
                    drag.cur_x = pos.x;
                    drag.cur_y = pos.y;
                    agg_gui::animation::request_draw();
                }
                EventResult::Ignored
            }
            Event::MouseUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.drag.is_some() {
                    if let Some(drag) = self.drag.as_ref() {
                        let is_click = (pos.x - drag.start_x).abs() <= CLICK_MOVE_RADIUS
                            && (pos.y - drag.start_y).abs() <= CLICK_MOVE_RADIUS;
                        if is_click {
                            let source_pile = drag.source_pile;
                            let start_idx = drag.start_idx;
                            self.drag = None;
                            if self.try_single_click_move(source_pile, start_idx) {
                                return EventResult::Consumed;
                            }
                        }
                    }
                    self.finish_drag(pos.x, pos.y);
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    fn needs_draw(&self) -> bool {
        // Visibility guard mirroring the framework default (agg-gui
        // `Widget::needs_draw`): an invisible subtree must NOT keep the
        // app in a continuous draw loop. Without this, a confetti burst
        // (or toast / animation) still live when the player leaves the
        // playfield — e.g. tapping Home mid-celebration flips the screen
        // to Title, where `is_visible` is false and `paint` never runs to
        // tick/drop the burst — would pin `wants_draw` true forever.
        if !self.is_visible() {
            return false;
        }
        self.drag.is_some()
            || !self.animations.is_empty()
            || !self.deck_animations.is_empty()
            || self.hint_anim.as_ref().is_some_and(|a| !a.done())
            || self.hint_pulse.as_ref().is_some_and(|p| !p.done())
            || self
                .hint_fade_out
                .as_ref()
                .is_some_and(|f| f.start_at.elapsed() < f.duration)
            || self.model.borrow().toast.is_some()
            || self.confetti.is_some()
    }
}
