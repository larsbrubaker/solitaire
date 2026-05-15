//! Playfield widget — owns ALL pointer input on the game board, hit-tests
//! piles directly, drives the drag-and-drop interaction, and emits
//! `Move`s into the active `DynGameSession`.
//!
//! Per CLAUDE.md "Drag is owned by GameWidget": pile-widgets do not
//! exist. `paint_pile` is a free function called from this widget's
//! `paint`.

mod animations;
mod banners;
mod pile_click;

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::cards::Card;
use crate::piles::{HitResult, PileId, PileKind, FAN_DOWN_FACE_UP};
use crate::render::{paint_card_at, paint_pile, CardSpriteAtlas};
use crate::session::Move;

use super::animation::{animated_deck_faces, CardAnim, DeckFace, DeckFlipAnim};
use super::app_model::{Screen, SharedModel};
use super::layout;

/// Chrome-aware playfield rect for the current viewport.
fn playfield_rect(bounds: Rect) -> Rect {
    layout::compute(Size::new(bounds.width, bounds.height)).playfield_rect
}

/// Stroke a rounded yellow outline over a card-sized rect — used by the
/// Spider Hint overlay to mark the recommended source run, destination,
/// or stock pile when the recommended action is a stock deal.
fn stroke_hint_rect(ctx: &mut dyn DrawCtx, x: f64, y: f64, w: f64, h: f64) {
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, crate::consts::CARD_CORNER_R);
    ctx.set_stroke_color(crate::render::HIGHLIGHT);
    ctx.set_line_width(4.0);
    ctx.stroke();
}

/// One-shot ghost preview kicked off by the Hint button — snapshots
/// the source cards plus their src + dst bottom-left positions, then
/// the widget interpolates the stack from src to dst and fades it out.
#[derive(Clone, Debug)]
struct HintAnim {
    cards: Vec<Card>,
    src_x: f64,
    src_y: f64,
    dst_x: f64,
    dst_y: f64,
    card_w: f64,
    card_h: f64,
    start_at: web_time::Instant,
    slide_dur: std::time::Duration,
    fade_dur: std::time::Duration,
}

impl HintAnim {
    /// Returns the cards' bottom-left for this frame and the alpha to
    /// paint them at. Once `done()` flips true the animation is dead.
    fn current(&self) -> (f64, f64, f64) {
        let el = self.start_at.elapsed().as_secs_f64().max(0.0);
        let slide_s = self.slide_dur.as_secs_f64();
        let fade_s = self.fade_dur.as_secs_f64();
        let slide_t = (el / slide_s).clamp(0.0, 1.0);
        // Ease-out cubic so the stack decelerates as it lands.
        let eased = 1.0 - (1.0 - slide_t).powi(3);
        let bx = self.src_x + (self.dst_x - self.src_x) * eased;
        let by = self.src_y + (self.dst_y - self.src_y) * eased;
        let fade_t = ((el - slide_s) / fade_s).clamp(0.0, 1.0);
        let alpha = 0.6 * (1.0 - fade_t);
        (bx, by, alpha)
    }

    fn done(&self) -> bool {
        self.start_at.elapsed() >= self.slide_dur + self.fade_dur
    }
}

#[derive(Clone, Debug)]
struct DragState {
    source_pile: PileId,
    /// First card index in source pile that's part of the drag.
    start_idx: usize,
    /// Snapshot of the cards being dragged (includes face_up flags).
    cards: Vec<Card>,
    /// Offset from the click point to the bottom-left of `cards[0]` in
    /// virtual coords. Lets the dragged stack follow the cursor at the
    /// same grab point through the whole motion.
    grab_dx: f64,
    grab_dy: f64,
    /// Latest cursor position in virtual coords.
    cur_x: f64,
    cur_y: f64,
    /// Initial pointer position; lets mouse-up distinguish a click from
    /// an intentional drag without preventing drag previews.
    start_x: f64,
    start_y: f64,
}

/// Window in which a second mouse-down at the same spot counts as a
/// double-click — used for the auto-send-to-foundation shortcut.
const DOUBLE_CLICK_MS: u128 = 350;
/// Maximum pointer drift between two clicks for them to be treated
/// as a double-click (in virtual pixels).
const DOUBLE_CLICK_RADIUS: f64 = 6.0;
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
        }
    }

    fn is_double_click(&self, vx: f64, vy: f64) -> bool {
        let Some((t, lx, ly)) = self.last_click else {
            return false;
        };
        t.elapsed().as_millis() <= DOUBLE_CLICK_MS
            && (lx - vx).abs() <= DOUBLE_CLICK_RADIUS
            && (ly - vy).abs() <= DOUBLE_CLICK_RADIUS
    }

    /// On a double-click of the topmost face-up card under the cursor,
    /// try every Foundation pile in declaration order and apply the
    /// first legal `src → foundation` move. Returns `true` if a move
    /// was applied.
    fn try_double_click_to_foundation(&mut self, vx: f64, vy: f64) -> bool {
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return false;
        };
        let piles = session.piles();
        let Some(hit) = piles.hit_test(vx, vy) else {
            return false;
        };
        let HitResult::Card {
            pile: src,
            card_idx,
        } = hit
        else {
            return false;
        };
        // Only the topmost card is auto-moveable.
        let p = piles.get(src);
        if card_idx + 1 != p.cards.len() {
            return false;
        }
        let top = &p.cards[card_idx];
        if !top.face_up {
            return false;
        }
        // Cancel any in-flight drag started by the first click of the pair.
        self.drag = None;
        // Snapshot foundation pile ids while we still hold an immutable
        // borrow of `piles` via `session.piles()`. After this block we
        // can call `session.try_apply` (which takes &mut self) without
        // tripping the borrow checker.
        let foundation_ids: Vec<PileId> = piles
            .iter()
            .filter(|p| p.kind == PileKind::Foundation && p.id != src)
            .map(|p| p.id)
            .collect();
        // Match the drag-drop branch: if removing the topmost card would
        // expose a face-down tableau card underneath, flip it. Without
        // this, double-clicking a card off auto-foundation leaves the
        // newly-revealed card face-down.
        let from_pile_kind = p.kind;
        let beneath_face_down =
            from_pile_kind == PileKind::Tableau && card_idx > 0 && !p.cards[card_idx - 1].face_up;
        for dst in foundation_ids {
            let mut m = Move::simple(src, 1, dst);
            if beneath_face_down {
                m = m.with_flip_source();
            }
            if session.legal_move(&m) {
                let Some(records) = session.try_apply_recording(m) else {
                    continue;
                };
                let now = web_time::Instant::now();
                animations::queue_recorded_move_animations(
                    &mut self.animations,
                    &records,
                    now,
                    true,
                );
                model.spider_hint = None;
                agg_gui::animation::request_draw();
                return true;
            }
        }
        false
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

    fn is_moms(&self) -> bool {
        self.model
            .borrow()
            .session
            .as_ref()
            .map(|s| s.game_slug() == "moms")
            .unwrap_or(false)
    }

    /// Mom's-Solitaire click handler. Looks up the clicked cell and
    /// asks `games::moms::resolve_click` what to do with it (start a
    /// king-pickup, fire a swap, or ignore). The session's normal
    /// `try_apply` runs the swap so undo / win-detection just work.
    fn try_moms_click(&mut self, vx: f64, vy: f64) -> bool {
        use crate::games::moms::{resolve_click, ClickResolution};
        let mut model = self.model.borrow_mut();
        let waiting = model.moms_waiting_king_at;
        // Phase 1: pure inspection — read piles, decide what the click
        // means. Borrow ends with `resolution`.
        let resolution = {
            let Some(session) = model.session.as_ref() else {
                return false;
            };
            let hit = session.piles().hit_test(vx, vy);
            let pile_id = match hit {
                Some(crate::piles::HitResult::Card { pile, .. }) => pile,
                Some(crate::piles::HitResult::EmptySlot { pile }) => pile,
                None => return false,
            };
            resolve_click(session.piles(), pile_id, waiting)
        };
        // Phase 2: apply mutation; the swap may also report a win.
        match resolution {
            ClickResolution::Ignored => false,
            ClickResolution::StartWaitingForKing(gap) => {
                model.moms_waiting_king_at = Some(gap);
                agg_gui::animation::request_draw();
                true
            }
            ClickResolution::ApplySwap(m) => {
                let won = {
                    let Some(session) = model.session.as_mut() else {
                        return false;
                    };
                    let Some(records) = session.try_apply_recording(m) else {
                        return false;
                    };
                    animations::queue_recorded_move_animations(
                        &mut self.animations,
                        &records,
                        web_time::Instant::now(),
                        true,
                    );
                    session.is_won()
                };
                model.moms_waiting_king_at = None;
                if won {
                    model.screen = Screen::Won;
                }
                agg_gui::animation::request_draw();
                true
            }
        }
    }

    fn try_start_drag(&mut self, vx: f64, vy: f64) -> bool {
        let model = self.model.borrow();
        let Some(session) = model.session.as_ref() else {
            return false;
        };
        let piles = session.piles();
        let Some(hit) = piles.hit_test(vx, vy) else {
            return false;
        };
        match hit {
            HitResult::EmptySlot { .. } => false,
            HitResult::Card { pile, card_idx } => {
                let p = piles.get(pile);
                let card = &p.cards[card_idx];
                if !card.face_up {
                    return false;
                }
                // For tableau piles you can pick up multiple cards (the
                // whole tail starting at card_idx). For other pile kinds
                // only the topmost card is draggable.
                let take_from = match p.kind {
                    PileKind::Tableau => card_idx,
                    _ => p.cards.len() - 1,
                };
                // The full tail must form a valid drag candidate. Rules
                // engine will reject if the run is invalid on drop, but
                // we still allow the drag visually.
                let cards: Vec<Card> = p.cards[take_from..].to_vec();
                let (cx, cy, _, _) = p.card_rect(take_from);
                self.drag = Some(DragState {
                    source_pile: pile,
                    start_idx: take_from,
                    cards,
                    grab_dx: vx - cx,
                    grab_dy: vy - cy,
                    cur_x: vx,
                    cur_y: vy,
                    start_x: vx,
                    start_y: vy,
                });
                drop(model); // release borrow before request_draw side-effects
                agg_gui::animation::request_draw();
                true
            }
        }
    }

    fn finish_drag(&mut self, vx: f64, vy: f64) {
        let Some(drag) = self.drag.take() else { return };
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return;
        };
        // Find the destination pile under the cursor (use the hit_test on
        // the dragged-card position rather than raw cursor — we anchor
        // off the dragged card[0]'s would-be origin). Card size comes
        // from the source pile (Mom's uses a different size than the
        // others).
        let src_pile = session.piles().get(drag.source_pile);
        let card_w = src_pile.card_w;
        let card_h = src_pile.card_h;
        let drag_card_x = vx - drag.grab_dx;
        let drag_card_y = vy - drag.grab_dy;
        let probe_x = drag_card_x + card_w / 2.0;
        let probe_y = drag_card_y + card_h / 2.0;
        let target = session.piles().hit_test(probe_x, probe_y);
        let target_pile: Option<PileId> = match target {
            Some(HitResult::Card { pile, .. }) => Some(pile),
            Some(HitResult::EmptySlot { pile }) => Some(pile),
            None => None,
        };
        let mut applied = false;
        if let Some(to) = target_pile {
            if to != drag.source_pile {
                let take = drag.cards.len() as u8;
                let from_pile = session.piles().get(drag.source_pile);
                // Mom's Solitaire's drag operation is a swap (the gap
                // and the card you drag onto it exchange places); every
                // other variant is a normal stack move with optional
                // auto-flip of the newly-exposed source card.
                let m = if session.game_slug() == "moms" {
                    Move::swap(drag.source_pile, to)
                } else {
                    let mut m = Move::simple(drag.source_pile, take, to);
                    if from_pile.kind == PileKind::Tableau && drag.start_idx > 0 {
                        let beneath = &from_pile.cards[drag.start_idx - 1];
                        if !beneath.face_up {
                            m = m.with_flip_source();
                        }
                    }
                    m
                };
                if let Some(records) = session.try_apply_recording(m) {
                    let now = web_time::Instant::now();
                    animations::queue_recorded_move_animations(
                        &mut self.animations,
                        &records,
                        now,
                        false,
                    );
                    applied = true;
                }
            }
        }
        // Win check.
        if session.is_won() {
            model.screen = Screen::Won;
        }
        if applied {
            model.spider_hint = None;
        }
        agg_gui::animation::request_draw();
    }

    fn try_single_click_move(&mut self, source_pile: PileId, start_idx: usize) -> bool {
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return false;
        };
        let Some(m) = session.single_click_move(source_pile, start_idx) else {
            return false;
        };
        let Some(records) = session.try_apply_recording(m) else {
            return false;
        };
        let now = web_time::Instant::now();
        animations::queue_recorded_move_animations(&mut self.animations, &records, now, true);
        if session.is_won() {
            model.screen = Screen::Won;
        }
        model.spider_hint = None;
        agg_gui::animation::request_draw();
        true
    }

    /// Paint the Spider Hint overlay: yellow outlines around the
    /// recommended source run and destination (or just the stock pile
    /// for a stock-deal hint). The companion ghost-card preview that
    /// animates source → destination on every Hint button press is
    /// painted by `paint_hint_animation`.
    fn paint_spider_hint_overlay(&self, ctx: &mut dyn DrawCtx) {
        let model = self.model.borrow();
        let Some(hint) = model.spider_hint else {
            return;
        };
        let Some(session) = model.session.as_ref() else {
            return;
        };
        let piles = session.piles();
        match hint {
            crate::games::spider::SpiderHint::Move {
                from,
                start_idx,
                take,
                to,
            } => {
                let src = piles.get(from);
                let take = take as usize;
                if start_idx >= src.cards.len() || take == 0 {
                    return;
                }
                let end_idx = (start_idx + take - 1).min(src.cards.len() - 1);
                let (hx, hy) = src.position_for(start_idx);
                let (_tx, ty) = src.position_for(end_idx);
                let x = hx;
                let y = ty;
                let w = src.card_w;
                let h = hy + src.card_h - ty;
                stroke_hint_rect(ctx, x, y, w, h);
                let dst = piles.get(to);
                let (dx, dy, dw, dh) = if dst.is_empty() {
                    dst.empty_slot_rect()
                } else {
                    dst.card_rect(dst.cards.len() - 1)
                };
                stroke_hint_rect(ctx, dx, dy, dw, dh);
            }
            crate::games::spider::SpiderHint::StockDeal { stock } => {
                let pile = piles.get(stock);
                let (sx, sy, sw, sh) = if pile.is_empty() {
                    pile.empty_slot_rect()
                } else {
                    pile.card_rect(pile.cards.len() - 1)
                };
                stroke_hint_rect(ctx, sx, sy, sw, sh);
            }
        }
    }

    /// Detect a fresh Hint button press (via `AppModel::spider_hint_seq`
    /// bumping) and snapshot the source/destination positions for a
    /// new ghost preview animation. Called every paint; cheap when
    /// nothing changed.
    fn tick_hint_animation(&mut self) {
        let model = self.model.borrow();
        let seq = model.spider_hint_seq;
        if seq == self.last_hint_seq {
            // Drop a finished one-shot.
            if self.hint_anim.as_ref().is_some_and(HintAnim::done) {
                self.hint_anim = None;
            }
            return;
        }
        self.last_hint_seq = seq;
        self.hint_anim = None;

        let Some(hint) = model.spider_hint else {
            return;
        };
        let crate::games::spider::SpiderHint::Move {
            from,
            start_idx,
            take,
            to,
        } = hint
        else {
            // Stock-deal hint: skip the ghost preview, the yellow rect
            // on the stock pile communicates the action well enough.
            return;
        };
        let Some(session) = model.session.as_ref() else {
            return;
        };
        let piles = session.piles();
        let src = piles.get(from);
        let take = take as usize;
        if start_idx >= src.cards.len() || take == 0 {
            return;
        }
        let cards: Vec<Card> = src.cards[start_idx..start_idx + take].to_vec();
        let (sx, sy) = src.position_for(start_idx);
        let dst = piles.get(to);
        // Where the head card would land after the move — Pile's
        // position_for evaluates the next slot using the dst's current
        // top card as `prev`, so it returns the right fan position for
        // both empty and non-empty destinations.
        let (dx, dy) = dst.position_for(dst.cards.len());
        self.hint_anim = Some(HintAnim {
            cards,
            src_x: sx,
            src_y: sy,
            dst_x: dx,
            dst_y: dy,
            card_w: src.card_w,
            card_h: src.card_h,
            start_at: web_time::Instant::now(),
            slide_dur: std::time::Duration::from_millis(550),
            fade_dur: std::time::Duration::from_millis(300),
        });
        agg_gui::animation::request_draw();
    }

    /// Paint the ghost-card preview at its current interpolated
    /// position with the in-flight alpha. No-op when no hint animation
    /// is active.
    fn paint_hint_animation(&self, ctx: &mut dyn DrawCtx) {
        let Some(anim) = self.hint_anim.as_ref() else {
            return;
        };
        if anim.done() {
            return;
        }
        let (bx, by, alpha) = anim.current();
        if alpha <= 0.0 {
            return;
        }
        let fan = anim.card_h * crate::piles::FAN_DOWN_FACE_UP;
        ctx.set_global_alpha(alpha);
        for (i, card) in anim.cards.iter().enumerate() {
            let y = by - i as f64 * fan;
            paint_card_at(ctx, card, bx, y, anim.card_w, anim.card_h, &self.atlas);
        }
        ctx.set_global_alpha(1.0);
    }

    fn paint_dragged(&self, ctx: &mut dyn DrawCtx, drag: &DragState) {
        let bx = drag.cur_x - drag.grab_dx;
        let by = drag.cur_y - drag.grab_dy;
        // Use the source pile's card dimensions so a Mom's Solitaire
        // card stays sized correctly while floating at the cursor.
        let model = self.model.borrow();
        let (card_w, card_h) = model
            .session
            .as_ref()
            .map(|s| {
                let p = s.piles().get(drag.source_pile);
                (p.card_w, p.card_h)
            })
            .unwrap_or((90.0, 126.0));
        drop(model);
        // Fan offset for the dragged stack matches the face-up tableau step.
        let fan = card_h * FAN_DOWN_FACE_UP;
        for (i, card) in drag.cards.iter().enumerate() {
            let y = by - i as f64 * fan;
            paint_card_at(ctx, card, bx, y, card_w, card_h, &self.atlas);
        }
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
        self.drag.is_some()
            || !self.animations.is_empty()
            || !self.deck_animations.is_empty()
            || self.hint_anim.as_ref().is_some_and(|a| !a.done())
    }
}
