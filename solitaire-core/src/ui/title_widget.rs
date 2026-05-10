//! Title screen — three buttons (Klondike, FreeCell, Spider) +
//! the toast for "coming soon" responses on unimplemented variants.
//!
//! Visible only while `model.screen == Screen::Title`.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::consts::{VIRTUAL_H, VIRTUAL_W};
use crate::games::GameKind;

use super::app_model::{Screen, SharedModel};

const BTN_W: f64 = 280.0;
const BTN_H: f64 = 70.0;
const BTN_GAP_Y: f64 = 18.0;

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

const KINDS: [GameKind; 3] = [
    GameKind::Klondike,
    GameKind::FreeCell,
    GameKind::Spider,
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

    fn button_rect(&self, idx: usize) -> (f64, f64, f64, f64) {
        let n = KINDS.len() as f64;
        let total_h = n * BTN_H + (n - 1.0) * BTN_GAP_Y;
        let start_y_top = (VIRTUAL_H - total_h) / 2.0 - 60.0;
        // Y-up: top of the FIRST button is at this Y. Each subsequent
        // button is BELOW (smaller numerical Y).
        let top_of_btn = start_y_top + total_h - idx as f64 * (BTN_H + BTN_GAP_Y);
        let y = top_of_btn - BTN_H;
        let x = (VIRTUAL_W - BTN_W) / 2.0;
        (x, y, BTN_W, BTN_H)
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
        ctx.set_fill_color(TITLE_COLOR);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(56.0);
        let label = "Solitaire";
        let m = ctx.measure_text(label);
        let w = m.map(|t| t.width).unwrap_or(0.0);
        let x = (VIRTUAL_W - w) / 2.0;
        // Title sits well above the buttons.
        let n = KINDS.len() as f64;
        let total_h = n * BTN_H + (n - 1.0) * BTN_GAP_Y;
        let buttons_top = (VIRTUAL_H - total_h) / 2.0 + total_h - 60.0;
        let y = buttons_top + 40.0;
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
        ctx.set_font_size(28.0);
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
        let x = (VIRTUAL_W - tw) / 2.0;
        let y = 80.0;
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
        // Allow toast time-out.
        self.model.borrow_mut().tick_toast();
        // Center the playfield horizontally inside whatever the OS gave us.
        // Buttons coordinate space is the virtual playfield (1024x720). We
        // letterbox-fit by translating so the playfield is centered in
        // self.bounds; coordinates inside paint_* helpers stay in virtual
        // space.
        let (tx, ty, scale) = playfield_transform(self.bounds);
        ctx.save();
        ctx.translate(tx, ty);
        ctx.scale(scale, scale);

        self.paint_title(ctx);
        for (i, kind) in KINDS.iter().enumerate() {
            self.paint_button(ctx, i, *kind, true);
        }
        self.paint_toast(ctx);

        ctx.restore();
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
                if self.click_at(vx, vy) {
                    agg_gui::animation::request_draw();
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            Event::MouseMove { pos } => {
                let (vx, vy) = screen_to_virtual(bounds, pos.x, pos.y);
                let mut new_hover = None;
                for (i, _) in KINDS.iter().enumerate() {
                    let (bx, by, bw, bh) = self.button_rect(i);
                    if vx >= bx && vx <= bx + bw && vy >= by && vy <= by + bh {
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

    fn needs_draw(&self) -> bool {
        // While a toast is showing we want regular redraws so it can fade
        // out on its own schedule. Cheap to always say true here.
        self.model.borrow().toast.is_some()
    }
}

/// Compute the (translation_x, translation_y, scale) needed to letterbox
/// the virtual VIRTUAL_W x VIRTUAL_H playfield inside `bounds`.
pub fn playfield_transform(bounds: Rect) -> (f64, f64, f64) {
    let sx = bounds.width / VIRTUAL_W;
    let sy = bounds.height / VIRTUAL_H;
    let scale = sx.min(sy);
    let used_w = VIRTUAL_W * scale;
    let used_h = VIRTUAL_H * scale;
    let tx = (bounds.width - used_w) / 2.0;
    let ty = (bounds.height - used_h) / 2.0;
    (tx, ty, scale)
}

/// Inverse of `playfield_transform` — convert a screen-bounds-local
/// pointer position to virtual playfield coordinates.
pub fn screen_to_virtual(bounds: Rect, px: f64, py: f64) -> (f64, f64) {
    let (tx, ty, scale) = playfield_transform(bounds);
    ((px - tx) / scale, (py - ty) / scale)
}
