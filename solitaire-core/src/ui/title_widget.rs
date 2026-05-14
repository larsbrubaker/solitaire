//! Title screen — four buttons (Klondike, FreeCell, Spider, Mom's
//! Solitaire) + the toast for transient status messages.
//!
//! Visible only while `model.screen == Screen::Title`. Lays out the
//! title heading + buttons directly in screen coordinates; the chrome
//! layout module decides what rect we get and we center inside it.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::games::GameKind;

use super::app_model::{Screen, SharedModel};

const BTN_W: f64 = 320.0;
const BTN_H: f64 = 84.0;
const BTN_GAP_Y: f64 = 22.0;
const TITLE_FONT_SIZE: f64 = 72.0;
const TITLE_BUTTONS_GAP: f64 = 42.0;

const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BG_DISABLED: Color = Color::from_rgba8(0x1f, 0x4d, 0x2e, 0x80);
const BTN_BORDER: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BTN_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const TITLE_COLOR: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const TOAST_BG: Color = Color::from_rgba8(0x10, 0x10, 0x10, 0xc0);
const TOAST_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

pub struct TitleWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    hover_idx: Option<usize>,
}

const KINDS: [GameKind; 4] = [
    GameKind::Klondike,
    GameKind::FreeCell,
    GameKind::Spider,
    GameKind::MomsSolitaire,
];

impl TitleWidget {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            hover_idx: None,
        }
    }

    /// Uniform scale applied to all title-screen dimensions when the
    /// viewport is too short for the full block (e.g. iPhone 12 Pro
    /// landscape, 844×390). 1.0 on desktop / portrait phones.
    fn fit_scale(&self) -> f64 {
        let n = KINDS.len() as f64;
        let block_h = n * BTN_H + (n - 1.0) * BTN_GAP_Y + TITLE_BUTTONS_GAP + TITLE_FONT_SIZE;
        // Leave a 24-px margin at top and bottom so the title and the
        // bottom button never paint flush against the chrome.
        let avail = (self.bounds.height - 48.0).max(1.0);
        (avail / block_h).min(1.0)
    }

    fn scaled(&self, v: f64) -> f64 {
        v * self.fit_scale()
    }

    /// Y-up screen-space rect for the title+buttons block, centered
    /// inside `self.bounds`.
    fn block_origin_y(&self) -> f64 {
        let n = KINDS.len() as f64;
        let s = self.fit_scale();
        let total_h = (n * BTN_H + (n - 1.0) * BTN_GAP_Y) * s;
        let block_h = total_h + (TITLE_BUTTONS_GAP + TITLE_FONT_SIZE) * s;
        self.bounds.y + (self.bounds.height - block_h) / 2.0
    }

    fn button_rect(&self, idx: usize) -> (f64, f64, f64, f64) {
        let start_y_top = self.block_origin_y();
        let n = KINDS.len() as f64;
        let s = self.fit_scale();
        let bh = BTN_H * s;
        let gap = BTN_GAP_Y * s;
        let total_h = n * bh + (n - 1.0) * gap;
        // Y-up: top of the FIRST button (Klondike) sits at the top of
        // the buttons portion of the block; subsequent buttons drop
        // by (bh + gap).
        let top_of_btn = start_y_top + total_h - idx as f64 * (bh + gap);
        let y = top_of_btn - bh;
        let bw = BTN_W * s;
        let x = self.bounds.x + (self.bounds.width - bw) / 2.0;
        (x, y, bw, bh)
    }

    fn click_at(&mut self, x: f64, y: f64) -> bool {
        for (i, kind) in KINDS.iter().enumerate() {
            let (bx, by, bw, bh) = self.button_rect(i);
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                let mut m = self.model.borrow_mut();
                m.start_game(*kind);
                return true;
            }
        }
        false
    }

    fn paint_title(&self, ctx: &mut dyn DrawCtx) {
        let s = self.fit_scale();
        ctx.set_fill_color(TITLE_COLOR);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(TITLE_FONT_SIZE * s);
        let label = "Solitaire";
        let m = ctx.measure_text(label);
        let w = m.map(|t| t.width).unwrap_or(0.0);
        let x = self.bounds.x + (self.bounds.width - w) / 2.0;
        // Title baseline sits scaled-TITLE_BUTTONS_GAP above the
        // topmost button (Klondike).
        let n = KINDS.len() as f64;
        let total_h = (n * BTN_H + (n - 1.0) * BTN_GAP_Y) * s;
        let start_y_top = self.block_origin_y();
        let klondike_top = start_y_top + total_h;
        let y = klondike_top + TITLE_BUTTONS_GAP * s;
        ctx.fill_text(label, x, y);
    }

    fn paint_button(&self, ctx: &mut dyn DrawCtx, idx: usize, kind: GameKind, enabled: bool) {
        let (x, y, w, h) = self.button_rect(idx);
        let bg = if !enabled {
            BTN_BG_DISABLED
        } else if self.hover_idx == Some(idx) {
            BTN_BG_HOVER
        } else {
            BTN_BG
        };
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 12.0);
        ctx.set_fill_color(bg);
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 12.0);
        ctx.set_stroke_color(BTN_BORDER);
        ctx.set_line_width(2.0);
        ctx.stroke();

        ctx.set_fill_color(BTN_TEXT);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(self.scaled(28.0));
        let label = kind.display_name();
        let Some(m) = ctx.measure_text(label) else {
            return;
        };
        let lx = x + (w - m.width) / 2.0;
        let ly = y + m.centered_baseline_y(h);
        ctx.fill_text(label, lx, ly);
    }

    fn paint_toast(&self, ctx: &mut dyn DrawCtx) {
        let toast = self.model.borrow().toast.clone();
        let Some((msg, _)) = toast else { return };
        let pad = 16.0;
        ctx.set_font(self.font.clone());
        ctx.set_font_size(20.0);
        let m = ctx.measure_text(&msg);
        let tw = m.map(|t| t.width).unwrap_or(220.0) + pad * 2.0;
        let th = 40.0;
        let x = self.bounds.x + (self.bounds.width - tw) / 2.0;
        let y = self.bounds.y + 80.0;
        ctx.begin_path();
        ctx.rounded_rect(x, y, tw, th, 8.0);
        ctx.set_fill_color(TOAST_BG);
        ctx.fill();
        ctx.set_fill_color(TOAST_TEXT);
        ctx.fill_text(&msg, x + pad, y + (th - 20.0) / 2.0);
    }
}

impl Widget for TitleWidget {
    fn type_name(&self) -> &'static str {
        "TitleWidget"
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
        self.model.borrow().screen == Screen::Title
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        self.model.borrow_mut().tick_toast();
        self.paint_title(ctx);
        for (i, kind) in KINDS.iter().enumerate() {
            self.paint_button(ctx, i, *kind, true);
        }
        self.paint_toast(ctx);
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
                if self.click_at(pos.x, pos.y) {
                    agg_gui::animation::request_draw();
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            Event::MouseMove { pos } => {
                let mut new_hover = None;
                for (i, _) in KINDS.iter().enumerate() {
                    let (bx, by, bw, bh) = self.button_rect(i);
                    if pos.x >= bx && pos.x <= bx + bw && pos.y >= by && pos.y <= by + bh {
                        new_hover = Some(i);
                        break;
                    }
                }
                if new_hover != self.hover_idx {
                    self.hover_idx = new_hover;
                    agg_gui::animation::request_draw();
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    // Toast is a static (non-animating) overlay that disappears 3 s
    // after `show_toast` was called.  We don't want a continuous
    // 60 fps repaint while it sits on screen — that would defeat the
    // reactive event loop the platform shells run for battery life.
    // Instead, surface the expiry as a `next_draw_deadline` so the
    // host wakes the loop exactly once when the toast should clear.
    fn next_draw_deadline(&self) -> Option<web_time::Instant> {
        self.model
            .borrow()
            .toast
            .as_ref()
            .map(|(_, started)| *started + super::app_model::TOAST_LIFETIME)
    }
}
