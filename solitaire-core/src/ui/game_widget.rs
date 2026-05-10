//! Playfield widget — owns ALL pointer input on the game board, hit-tests
//! piles directly, drives the drag-and-drop interaction, and emits
//! `Move`s into the active `DynGameSession`.
//!
//! Per CLAUDE.md "Drag is owned by GameWidget": pile-widgets do not
//! exist. `paint_pile` is a free function called from this widget's
//! `paint`.

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::cards::Card;
use crate::consts::{CARD_H, CARD_W};
use crate::piles::{HitResult, PileId, PileKind};
use crate::render::{paint_card_at, paint_pile, CardSpriteAtlas};
use crate::session::Move;

use super::app_model::{Screen, SharedModel};
use super::title_widget::{playfield_transform, screen_to_virtual};

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
}

/// Window in which a second mouse-down at the same spot counts as a
/// double-click — used for the auto-send-to-foundation shortcut.
const DOUBLE_CLICK_MS: u128 = 350;
/// Maximum pointer drift between two clicks for them to be treated
/// as a double-click (in virtual pixels).
const DOUBLE_CLICK_RADIUS: f64 = 6.0;

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
            if session.legal_move(&m) && session.try_apply(m) {
                agg_gui::animation::request_draw();
                return true;
            }
        }
        false
    }

    /// Rebuild the sprite atlas at the current effective render scale
    /// (playfield scale × device DPR) AND the active variant's per-
    /// pile card dimensions so each sprite's pixel count matches its
    /// on-screen physical size 1:1. Rebuilds on either change.
    fn ensure_atlas_for(&mut self, playfield_scale: f64) {
        let target = (playfield_scale * agg_gui::device_scale()).max(0.5);
        // Active variant's card size — Mom's Solitaire shrinks every
        // pile to 70×98; the others use the standard 90×126. Read
        // from any pile in the current session (they all match).
        let (card_w, card_h) = self
            .model
            .borrow()
            .session
            .as_ref()
            .and_then(|s| s.piles().iter().next().map(|p| (p.card_w, p.card_h)))
            .unwrap_or((CARD_W, CARD_H));
        let scale_unchanged = (target - self.atlas.render_scale).abs() < 0.02;
        let dims_unchanged = (card_w - self.atlas.card_w_logical).abs() < 0.5
            && (card_h - self.atlas.card_h_logical).abs() < 0.5;
        if scale_unchanged && dims_unchanged {
            return;
        }
        let t0 = web_time::Instant::now();
        self.atlas = CardSpriteAtlas::build(&self.font, card_w, card_h, target);
        eprintln!(
            "solitaire: rebuilt atlas at scale {:.3} for {}×{} cards ({}×{} px) in {:.1} ms",
            target,
            card_w,
            card_h,
            self.atlas.px_w,
            self.atlas.px_h,
            t0.elapsed().as_secs_f64() * 1000.0
        );
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
                    if !session.try_apply(m) {
                        return false;
                    }
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
                });
                drop(model); // release borrow before request_draw side-effects
                agg_gui::animation::request_draw();
                true
            }
        }
    }

    fn try_pile_click(&mut self, vx: f64, vy: f64) -> bool {
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return false;
        };
        let piles = session.piles();
        let Some(hit) = piles.hit_test(vx, vy) else {
            return false;
        };
        let pile_id = match hit {
            HitResult::Card { pile, .. } => pile,
            HitResult::EmptySlot { pile } => pile,
        };
        let moves = session.on_pile_click(pile_id);
        if moves.is_empty() {
            return false;
        }
        let mut applied_any = false;
        for m in moves {
            if session.try_apply(m) {
                applied_any = true;
            } else {
                break;
            }
        }
        if applied_any {
            agg_gui::animation::request_draw();
        }
        applied_any
    }

    fn finish_drag(&mut self, vx: f64, vy: f64) {
        let Some(drag) = self.drag.take() else { return };
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return;
        };
        // Find the destination pile under the cursor (use the hit_test on
        // the dragged-card position rather than raw cursor — we anchor
        // off the dragged card[0]'s would-be origin).
        let drag_card_x = vx - drag.grab_dx;
        let drag_card_y = vy - drag.grab_dy;
        let probe_x = drag_card_x + CARD_W / 2.0;
        let probe_y = drag_card_y + CARD_H / 2.0;
        let target = session.piles().hit_test(probe_x, probe_y);
        let target_pile: Option<PileId> = match target {
            Some(HitResult::Card { pile, .. }) => Some(pile),
            Some(HitResult::EmptySlot { pile }) => Some(pile),
            None => None,
        };
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
                session.try_apply(m);
            }
        }
        // Win check.
        if session.is_won() {
            model.screen = Screen::Won;
        }
        agg_gui::animation::request_draw();
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
            .unwrap_or((CARD_W, CARD_H));
        drop(model);
        for (i, card) in drag.cards.iter().enumerate() {
            let y = by - i as f64 * crate::consts::TABLEAU_FAN_DOWN;
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
        let t0 = web_time::Instant::now();
        let (tx, ty, scale) = playfield_transform(self.bounds);
        self.ensure_atlas_for(scale);
        ctx.save();
        ctx.translate(tx, ty);
        ctx.scale(scale, scale);

        // Paint piles.
        let model = self.model.borrow();
        if let Some(session) = model.session.as_ref() {
            let piles = session.piles();
            for pile in piles.iter() {
                let hide_from = self
                    .drag
                    .as_ref()
                    .filter(|d| d.source_pile == pile.id)
                    .map(|d| d.start_idx);
                paint_pile(ctx, pile, hide_from, &self.atlas);
            }
        }
        drop(model);

        // Paint dragged cards on top.
        if let Some(drag) = self.drag.clone() {
            self.paint_dragged(ctx, &drag);
        }

        // Win banner.
        if self.model.borrow().screen == Screen::Won {
            paint_win_banner(ctx, &self.font);
        }

        // Mom's Solitaire: while a col-0 gap is armed, prompt the
        // user to pick a King.
        if self.model.borrow().moms_waiting_king_at.is_some() {
            paint_moms_prompt(ctx, &self.font, "Select a King for the empty slot");
        }

        ctx.restore();

        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        if ms > 30.0 {
            eprintln!("solitaire: GameWidget paint took {:.1} ms", ms);
        }
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        let bounds = self.bounds;
        match event {
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let (vx, vy) = screen_to_virtual(bounds, pos.x, pos.y);
                // Detect double-click against the previous MouseDown
                // BEFORE updating the timestamp.
                let is_double = self.is_double_click(vx, vy);
                self.last_click = Some((web_time::Instant::now(), vx, vy));

                // Mom's Solitaire is click-only — the player clicks a
                // gap and the engine finds the matching card. No
                // drag, no double-click-to-foundation (no foundation).
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
                // Pile-click handler first (stock dispense / recycle).
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
                    let (vx, vy) = screen_to_virtual(bounds, pos.x, pos.y);
                    drag.cur_x = vx;
                    drag.cur_y = vy;
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
                    let (vx, vy) = screen_to_virtual(bounds, pos.x, pos.y);
                    self.finish_drag(vx, vy);
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    fn needs_draw(&self) -> bool {
        self.drag.is_some()
    }
}

fn paint_win_banner(ctx: &mut dyn DrawCtx, font: &Arc<Font>) {
    use crate::consts::{VIRTUAL_H, VIRTUAL_W};
    use agg_gui::color::Color;
    let bg = Color::from_rgba8(0x10, 0x10, 0x10, 0xc8);
    let fg = Color::from_rgb8(0xff, 0xd7, 0x00);
    let pad = 30.0;
    let label = "You Won!";
    ctx.set_font(font.clone());
    ctx.set_font_size(56.0);
    let m = ctx.measure_text(label);
    let lw = m.map(|t| t.width).unwrap_or(280.0);
    let bw = lw + pad * 2.0;
    let bh = 100.0;
    let bx = (VIRTUAL_W - bw) / 2.0;
    let by = (VIRTUAL_H - bh) / 2.0;
    ctx.begin_path();
    ctx.rounded_rect(bx, by, bw, bh, 14.0);
    ctx.set_fill_color(bg);
    ctx.fill();
    ctx.set_fill_color(fg);
    ctx.fill_text(label, bx + pad, by + (bh - 56.0) / 2.0);
}

/// Status banner for Mom's Solitaire's "select a King for the empty
/// slot" prompt. Painted near the top of the playfield, similar in
/// style to the C# original's instruction banner.
fn paint_moms_prompt(ctx: &mut dyn DrawCtx, font: &Arc<Font>, label: &str) {
    use crate::consts::{VIRTUAL_H, VIRTUAL_W};
    use agg_gui::color::Color;
    // Soft warm-pink fill matching the C# original's "needs a King"
    // banner (`new Color(0xf8, 0x89, 0x78)`), with a dark outline so it
    // stays readable on top of the green felt and the gap cells.
    let bg = Color::from_rgba8(0xf8, 0x89, 0x78, 0xf0);
    let border = Color::from_rgb8(0x20, 0x20, 0x20);
    let fg = Color::from_rgb8(0x10, 0x10, 0x10);
    let pad_x = 18.0;
    let bh = 32.0;
    let font_size = 16.0;
    ctx.set_font(font.clone());
    ctx.set_font_size(font_size);
    let m = ctx.measure_text(label);
    let lw = m.map(|t| t.width).unwrap_or(240.0);
    let bw = lw + pad_x * 2.0;
    let bx = (VIRTUAL_W - bw) / 2.0;
    // Y-up: place the banner near the TOP of the playfield, just
    // below where the menu bar lands in window space.
    let by = VIRTUAL_H - bh - 8.0;
    ctx.begin_path();
    ctx.rounded_rect(bx, by, bw, bh, 6.0);
    ctx.set_fill_color(bg);
    ctx.fill();
    ctx.begin_path();
    ctx.rounded_rect(bx, by, bw, bh, 6.0);
    ctx.set_stroke_color(border);
    ctx.set_line_width(1.5);
    ctx.stroke();
    ctx.set_fill_color(fg);
    if let Some(m) = ctx.measure_text(label) {
        let baseline = by + m.centered_baseline_y(bh);
        ctx.fill_text(label, bx + pad_x, baseline);
    }
}
