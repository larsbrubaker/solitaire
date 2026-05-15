//! "Play deal number…" modal — accepts a decimal Microsoft FreeCell
//! game number or a `u64` seed (decimal / 0x-prefixed hex) for the
//! other variants, and starts a new deal at that number. Mirrors
//! [`ConfirmDialog`]'s overall structure: a scrim, a centred panel,
//! a typed-input field, Cancel + Play buttons, plus an inline
//! error line for invalid input.
//!
//! Doesn't use `agg_gui::widgets::TextField` — the Widget trait's
//! generic accessor doesn't expose typed input/output, and routing
//! events through a child TextField while keeping the rest of the
//! dialog functional adds non-trivial plumbing for what's a single
//! short numeric field. Instead we handle keystrokes inline: digits
//! / hex / `x` append, Backspace pops, Enter/Escape commit/cancel.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use super::app_model::SharedModel;

const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x90);
const PANEL_BG: Color = Color::from_rgb8(0x1a, 0x2c, 0x20);
const PANEL_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const TITLE_TEXT: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BODY_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const HINT_TEXT: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0xb0);
const ERROR_TEXT: Color = Color::from_rgb8(0xff, 0x80, 0x80);
const FIELD_BG: Color = Color::from_rgb8(0x0c, 0x1c, 0x12);
const FIELD_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);

const PANEL_W: f64 = 460.0;
const PANEL_H: f64 = 230.0;
const PAD: f64 = 22.0;
const BTN_W: f64 = 130.0;
const BTN_H: f64 = 38.0;
const BTN_GAP: f64 = 16.0;
const FIELD_H: f64 = 40.0;
const MAX_INPUT_LEN: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Cancel,
    Play,
}

pub struct PlayDealDialog {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    hover: Option<Button>,
    error: Option<&'static str>,
    input: String,
    last_open: bool,
}

impl PlayDealDialog {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            hover: None,
            error: None,
            input: String::new(),
            last_open: false,
        }
    }

    fn is_open(&self) -> bool {
        self.model.borrow().play_deal_dialog_open
    }

    fn panel_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if self.bounds.width <= 0.0 || self.bounds.height <= 0.0 {
            return None;
        }
        let w = PANEL_W.min(self.bounds.width - 32.0).max(280.0);
        let h = PANEL_H.min(self.bounds.height - 32.0).max(180.0);
        Some((
            (self.bounds.width - w) / 2.0,
            (self.bounds.height - h) / 2.0,
            w,
            h,
        ))
    }

    fn button_rect(&self, button: Button) -> Option<(f64, f64, f64, f64)> {
        let (px, py, pw, _) = self.panel_rect()?;
        let total = BTN_W * 2.0 + BTN_GAP;
        let y = py + PAD;
        let cancel_x = px + (pw - total) / 2.0;
        let x = match button {
            Button::Cancel => cancel_x,
            Button::Play => cancel_x + BTN_W + BTN_GAP,
        };
        Some((x, y, BTN_W, BTN_H))
    }

    fn field_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (px, py, pw, _) = self.panel_rect()?;
        Some((px + PAD, py + BTN_H + PAD * 2.0, pw - PAD * 2.0, FIELD_H))
    }

    fn hit_button(&self, x: f64, y: f64) -> Option<Button> {
        [Button::Cancel, Button::Play]
            .into_iter()
            .find(|&b| self.button_rect(b).is_some_and(|r| point_in(r, x, y)))
    }

    fn click_button(&mut self, button: Button) {
        self.hover = None;
        match button {
            Button::Cancel => {
                self.error = None;
                self.input.clear();
                self.model.borrow_mut().cancel_play_deal_dialog();
            }
            Button::Play => {
                let input = self.input.clone();
                match self.model.borrow_mut().commit_play_deal_dialog(&input) {
                    Ok(()) => {
                        self.error = None;
                        self.input.clear();
                    }
                    Err(msg) => {
                        self.error = Some(msg);
                    }
                }
            }
        }
        agg_gui::animation::request_draw();
    }

    fn key_to_char(&mut self, c: char) {
        if self.input.len() >= MAX_INPUT_LEN {
            return;
        }
        let ok = c.is_ascii_digit()
            || c == 'x'
            || c == 'X'
            || matches!(c, 'a'..='f' | 'A'..='F');
        if ok {
            self.input.push(c);
            self.error = None;
            agg_gui::animation::request_draw();
        }
    }

    fn paint_button(&self, ctx: &mut dyn DrawCtx, button: Button, label: &str) {
        let Some((x, y, w, h)) = self.button_rect(button) else {
            return;
        };
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 8.0);
        ctx.set_fill_color(if self.hover == Some(button) {
            BTN_BG_HOVER
        } else {
            BTN_BG
        });
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 8.0);
        ctx.set_stroke_color(BTN_BORDER);
        ctx.set_line_width(1.5);
        ctx.stroke();

        ctx.set_fill_color(BODY_TEXT);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(17.0);
        if let Some(m) = ctx.measure_text(label) {
            ctx.fill_text(label, x + (w - m.width) / 2.0, y + m.centered_baseline_y(h));
        }
    }
}

fn point_in(rect: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
    let (rx, ry, rw, rh) = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

impl Widget for PlayDealDialog {
    fn type_name(&self) -> &'static str {
        "PlayDealDialog"
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
        self.is_open()
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        // Reset transient state on each open so re-launching the
        // dialog after an error or partial entry starts fresh.
        let open = self.is_open();
        if open && !self.last_open {
            self.input.clear();
            self.error = None;
            self.hover = None;
        }
        self.last_open = open;
        if !open {
            return;
        }
        let Some((px, py, pw, ph)) = self.panel_rect() else {
            return;
        };

        ctx.begin_path();
        ctx.rect(0.0, 0.0, self.bounds.width, self.bounds.height);
        ctx.set_fill_color(SCRIM);
        ctx.fill();

        ctx.begin_path();
        ctx.rounded_rect(px, py, pw, ph, 12.0);
        ctx.set_fill_color(PANEL_BG);
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(px, py, pw, ph, 12.0);
        ctx.set_stroke_color(PANEL_BORDER);
        ctx.set_line_width(1.5);
        ctx.stroke();

        ctx.set_font(self.font.clone());
        ctx.set_fill_color(TITLE_TEXT);
        ctx.set_font_size(22.0);
        let title = "Play deal number";
        if let Some(m) = ctx.measure_text(title) {
            ctx.fill_text(title, px + (pw - m.width) / 2.0, py + ph - PAD - 22.0);
        }

        let hint = "Decimal (e.g. 11234) or 0x… hex seed";
        ctx.set_fill_color(HINT_TEXT);
        ctx.set_font_size(13.0);
        if let Some(m) = ctx.measure_text(hint) {
            let hint_y = py + ph - PAD - 22.0 - 26.0;
            ctx.fill_text(hint, px + (pw - m.width) / 2.0, hint_y);
        }

        // Input field.
        if let Some((fx, fy, fw, fh)) = self.field_rect() {
            ctx.begin_path();
            ctx.rounded_rect(fx, fy, fw, fh, 6.0);
            ctx.set_fill_color(FIELD_BG);
            ctx.fill();
            ctx.begin_path();
            ctx.rounded_rect(fx, fy, fw, fh, 6.0);
            ctx.set_stroke_color(FIELD_BORDER);
            ctx.set_line_width(1.0);
            ctx.stroke();
            let display: String = if self.input.is_empty() {
                "_".to_string()
            } else {
                format!("{}_", self.input)
            };
            ctx.set_fill_color(BODY_TEXT);
            ctx.set_font_size(20.0);
            if let Some(m) = ctx.measure_text(&display) {
                ctx.fill_text(&display, fx + 10.0, fy + m.centered_baseline_y(fh));
            }
        }

        if let Some(err) = self.error {
            ctx.set_fill_color(ERROR_TEXT);
            ctx.set_font_size(13.0);
            if let Some(m) = ctx.measure_text(err) {
                let (_, fy, _, _) = self.field_rect().unwrap_or((0.0, 0.0, 0.0, 0.0));
                ctx.fill_text(err, px + (pw - m.width) / 2.0, fy - 20.0);
            }
        }

        self.paint_button(ctx, Button::Cancel, "Cancel");
        self.paint_button(ctx, Button::Play, "Play");
    }

    fn hit_test(&self, _local_pos: Point) -> bool {
        self.is_visible()
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
                if let Some(button) = self.hit_button(pos.x, pos.y) {
                    self.click_button(button);
                }
                EventResult::Consumed
            }
            Event::MouseMove { pos } => {
                let hover = self.hit_button(pos.x, pos.y);
                if hover != self.hover {
                    self.hover = hover;
                    agg_gui::animation::request_draw();
                }
                EventResult::Consumed
            }
            Event::KeyDown { key, .. } => {
                match key {
                    Key::Escape => self.click_button(Button::Cancel),
                    Key::Enter => self.click_button(Button::Play),
                    Key::Backspace => {
                        self.input.pop();
                        self.error = None;
                        agg_gui::animation::request_draw();
                    }
                    Key::Char(c) => {
                        self.key_to_char(*c);
                    }
                    _ => {}
                }
                EventResult::Consumed
            }
            _ => EventResult::Consumed,
        }
    }
}
