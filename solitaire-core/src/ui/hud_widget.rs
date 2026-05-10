//! Bottom-of-window HUD strip — Undo / New Deal / Back-to-title buttons,
//! plus the active variant's name. Visible whenever a game is active.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::consts::{HUD_HEIGHT, VIRTUAL_W};

use super::app_model::{Screen, SharedModel};
use super::title_widget::{playfield_transform, screen_to_virtual};

const HUD_BG: Color = Color::from_rgba8(0x09, 0x52, 0x2c, 0xe0);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);
const TXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    Undo,
    NewDeal,
    Home,
}

const BTNS: [Btn; 3] = [Btn::Undo, Btn::NewDeal, Btn::Home];

fn btn_label(b: Btn) -> &'static str {
    match b {
        Btn::Undo => "Undo",
        Btn::NewDeal => "New Deal",
        Btn::Home => "Title",
    }
}

const BTN_W: f64 = 120.0;
const BTN_H: f64 = 36.0;
const BTN_GAP: f64 = 12.0;

pub struct HudWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    hover: Option<Btn>,
}

impl HudWidget {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            hover: None,
        }
    }

    fn btn_rect(&self, idx: usize) -> (f64, f64, f64, f64) {
        let strip_y = 0.0; // Y-up: strip sits at the bottom.
        let total_w = BTNS.len() as f64 * BTN_W + (BTNS.len() as f64 - 1.0) * BTN_GAP;
        let start_x = (VIRTUAL_W - total_w) / 2.0;
        let x = start_x + idx as f64 * (BTN_W + BTN_GAP);
        let y = strip_y + (HUD_HEIGHT - BTN_H) / 2.0;
        (x, y, BTN_W, BTN_H)
    }

    fn click_at(&mut self, vx: f64, vy: f64) -> bool {
        for (i, b) in BTNS.iter().enumerate() {
            let (x, y, w, h) = self.btn_rect(i);
            if vx >= x && vx <= x + w && vy >= y && vy <= y + h {
                let mut model = self.model.borrow_mut();
                match b {
                    Btn::Undo => {
                        if let Some(s) = model.session.as_mut() {
                            s.try_undo();
                        }
                    }
                    Btn::NewDeal => {
                        // Restart the same variant.
                        if model.kind == Some(crate::games::GameKind::Klondike) {
                            model.start_klondike();
                        }
                    }
                    Btn::Home => {
                        model.back_to_title();
                    }
                }
                agg_gui::animation::request_draw();
                return true;
            }
        }
        false
    }

    fn paint_strip(&self, ctx: &mut dyn DrawCtx) {
        ctx.set_fill_color(HUD_BG);
        ctx.rect(0.0, 0.0, VIRTUAL_W, HUD_HEIGHT);
        ctx.fill();
    }

    fn paint_btn(&self, ctx: &mut dyn DrawCtx, idx: usize, b: Btn) {
        let (x, y, w, h) = self.btn_rect(idx);
        let bg = if self.hover == Some(b) {
            BTN_BG_HOVER
        } else {
            BTN_BG
        };
        ctx.set_fill_color(bg);
        ctx.rounded_rect(x, y, w, h, 8.0);
        ctx.fill();
        ctx.set_stroke_color(BTN_BORDER);
        ctx.set_line_width(1.5);
        ctx.rounded_rect(x, y, w, h, 8.0);
        ctx.stroke();

        ctx.set_fill_color(TXT);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(18.0);
        let label = btn_label(b);
        let m = ctx.measure_text(label);
        let lw = m.map(|t| t.width).unwrap_or(0.0);
        let lx = x + (w - lw) / 2.0;
        let ly = y + (h - 18.0) / 2.0;
        ctx.fill_text(label, lx, ly);
    }
}

impl Widget for HudWidget {
    fn type_name(&self) -> &'static str {
        "HudWidget"
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
        let (tx, ty, scale) = playfield_transform(self.bounds);
        ctx.save();
        ctx.translate(tx, ty);
        ctx.scale(scale, scale);

        self.paint_strip(ctx);
        for (i, b) in BTNS.iter().enumerate() {
            self.paint_btn(ctx, i, *b);
        }
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
                if vy < HUD_HEIGHT && self.click_at(vx, vy) {
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            Event::MouseMove { pos } => {
                let (vx, vy) = screen_to_virtual(bounds, pos.x, pos.y);
                let mut new_hover = None;
                if vy < HUD_HEIGHT {
                    for (i, b) in BTNS.iter().enumerate() {
                        let (x, y, w, h) = self.btn_rect(i);
                        if vx >= x && vx <= x + w && vy >= y && vy <= y + h {
                            new_hover = Some(*b);
                            break;
                        }
                    }
                }
                if new_hover != self.hover {
                    self.hover = new_hover;
                    agg_gui::animation::request_draw();
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
