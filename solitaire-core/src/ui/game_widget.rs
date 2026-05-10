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

pub struct GameWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    atlas: Arc<CardSpriteAtlas>,
    drag: Option<DragState>,
}

impl GameWidget {
    pub fn new(model: SharedModel, font: Arc<Font>, atlas: Arc<CardSpriteAtlas>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            atlas,
            drag: None,
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
        let Some(m) = session.on_pile_click(pile_id) else {
            return false;
        };
        let applied = session.try_apply(m);
        if applied {
            agg_gui::animation::request_draw();
        }
        applied
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
                // If the source pile is a tableau and removing the dragged
                // tail would expose a face-down card on top, set
                // flip_source_after.
                let mut m = Move::simple(drag.source_pile, take, to);
                if from_pile.kind == PileKind::Tableau && drag.start_idx > 0 {
                    let beneath = &from_pile.cards[drag.start_idx - 1];
                    if !beneath.face_up {
                        m = m.with_flip_source();
                    }
                }
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
        for (i, card) in drag.cards.iter().enumerate() {
            let y = by - i as f64 * crate::consts::TABLEAU_FAN_DOWN;
            paint_card_at(ctx, card, bx, y, &self.atlas);
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
