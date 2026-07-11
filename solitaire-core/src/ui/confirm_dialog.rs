//! Confirmation dialog for actions that abandon the current game.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::{Label, LabelAlign};

use super::app_model::{ConfirmAction, SharedModel};

const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x90);
const PANEL_BG: Color = Color::from_rgb8(0x1a, 0x2c, 0x20);
const PANEL_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const TITLE_TEXT: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BODY_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);

const PANEL_W: f64 = 430.0;
const PANEL_H: f64 = 210.0;
const PAD: f64 = 22.0;
const BTN_W: f64 = 130.0;
const BTN_H: f64 = 38.0;
const BTN_GAP: f64 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Cancel,
    Confirm,
}

pub struct ConfirmDialog {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    hover: Option<Button>,
}

impl ConfirmDialog {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: vec![Box::new(
                Label::new("", font.clone())
                    .with_font_size(16.0)
                    .with_color(BODY_TEXT)
                    .with_align(LabelAlign::Center)
                    .with_wrap(true),
            )],
            model,
            font,
            hover: None,
        }
    }

    fn action(&self) -> Option<ConfirmAction> {
        self.model.borrow().confirm
    }

    fn panel_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if self.bounds.width <= 0.0 || self.bounds.height <= 0.0 {
            return None;
        }
        let w = PANEL_W.min(self.bounds.width - 32.0).max(260.0);
        let h = PANEL_H.min(self.bounds.height - 32.0).max(160.0);
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
            Button::Confirm => cancel_x + BTN_W + BTN_GAP,
        };
        Some((x, y, BTN_W, BTN_H))
    }

    fn body_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (px, py, pw, ph) = self.panel_rect()?;
        Some((
            px + PAD,
            py + BTN_H + PAD * 2.0,
            pw - PAD * 2.0,
            ph - BTN_H - PAD * 4.5,
        ))
    }

    fn hit_button(&self, x: f64, y: f64) -> Option<Button> {
        [Button::Cancel, Button::Confirm]
            .into_iter()
            .find(|&b| self.button_rect(b).is_some_and(|r| point_in(r, x, y)))
    }

    fn confirm_label(action: ConfirmAction) -> &'static str {
        match action {
            ConfirmAction::NewDeal(_) => "New Deal",
            ConfirmAction::MainMenu => "Main Menu",
            ConfirmAction::ApplyKlondikeDrawCount(_)
            | ConfirmAction::ApplySpiderSuitCount(_)
            | ConfirmAction::ApplySpiderWinnableOnly(_)
            | ConfirmAction::ApplyFreeCellWinnableOnly(_)
            | ConfirmAction::ApplyKlondikeWinnableOnly(_) => "Apply",
        }
    }

    fn message(action: ConfirmAction) -> &'static str {
        match action {
            ConfirmAction::NewDeal(_) => "This will abandon the current game and start a new deal.",
            ConfirmAction::MainMenu => {
                "This will abandon the current game and return to the main menu."
            }
            ConfirmAction::ApplyKlondikeDrawCount(_)
            | ConfirmAction::ApplySpiderSuitCount(_)
            | ConfirmAction::ApplySpiderWinnableOnly(_)
            | ConfirmAction::ApplyFreeCellWinnableOnly(_)
            | ConfirmAction::ApplyKlondikeWinnableOnly(_) => {
                "Applying this setting will abandon the current game and start a new deal."
            }
        }
    }

    fn sync_body_label(&mut self, action: ConfirmAction) {
        let message = Self::message(action);
        if let Some(child) = self.children.first_mut() {
            child.set_label_text(message);
        }
        self.update_child_bounds();
    }

    fn update_child_bounds(&mut self) {
        let Some((x, y, w, h)) = self.body_rect() else {
            return;
        };
        if let Some(child) = self.children.first_mut() {
            let measured = child.layout(Size::new(w, h));
            let child_h = measured.height.min(h).max(1.0);
            child.set_bounds(Rect::new(x, y + (h - child_h) / 2.0, w, child_h));
        }
    }

    fn click_button(&mut self, button: Button) {
        match button {
            Button::Cancel => self.model.borrow_mut().cancel_pending_action(),
            Button::Confirm => self.model.borrow_mut().confirm_pending_action(),
        }
        self.hover = None;
        agg_gui::animation::request_draw();
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
    x >= rect.0 && x <= rect.0 + rect.2 && y >= rect.1 && y <= rect.1 + rect.3
}

impl Widget for ConfirmDialog {
    fn type_name(&self) -> &'static str {
        "ConfirmDialog"
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.update_child_bounds();
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }

    fn layout(&mut self, available: Size) -> Size {
        self.update_child_bounds();
        available
    }

    fn is_visible(&self) -> bool {
        self.action().is_some()
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let Some(action) = self.action() else {
            return;
        };
        self.sync_body_label(action);
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
        let title = "Abandon current game?";
        if let Some(m) = ctx.measure_text(title) {
            ctx.fill_text(title, px + (pw - m.width) / 2.0, py + ph - PAD - 22.0);
        }

        self.paint_button(ctx, Button::Cancel, "Cancel");
        self.paint_button(ctx, Button::Confirm, Self::confirm_label(action));
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
                    Key::Enter => self.click_button(Button::Confirm),
                    _ => {}
                }
                EventResult::Consumed
            }
            _ => EventResult::Consumed,
        }
    }
}
