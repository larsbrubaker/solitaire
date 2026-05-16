//! "Play deal number…" modal — accepts a decimal Microsoft FreeCell
//! game number or a `u64` seed (decimal / 0x-prefixed hex) for the
//! other variants, and starts a new deal at that number.
//!
//! The numeric input is an [`agg_gui::widgets::TextField`] (with a
//! `char_filter` restricting input to decimal digits, hex letters,
//! and the `x`/`X` separator used by 0x-prefixed seeds). The dialog
//! still owns the scrim, the panel, the title/hint copy, the
//! Cancel/Play buttons, and the inline error line. Keystrokes
//! reach the TextField via manual forwarding from `on_event`
//! because the modal-path routing stops at the dialog level — see
//! `has_active_modal` + `forward_key_to_field`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::TextField;

use super::app_model::SharedModel;

const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x90);
const PANEL_BG: Color = Color::from_rgb8(0x1a, 0x2c, 0x20);
const PANEL_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const TITLE_TEXT: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BODY_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const HINT_TEXT: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0xb0);
const ERROR_TEXT: Color = Color::from_rgb8(0xff, 0x80, 0x80);
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
    /// Backing buffer that the TextField reads from and writes to via
    /// `with_text_cell`. Letting the cell live outside the TextField
    /// means the dialog can clear it on open / commit without having
    /// to take a `&mut` borrow on the child widget.
    text_cell: Rc<RefCell<String>>,
    last_open: bool,
}

impl PlayDealDialog {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let text_cell = Rc::new(RefCell::new(String::new()));
        // The TextField stays at index 0 in `children` for the
        // lifetime of the dialog — the inline forwarders rely on
        // that position. `on_enter` captures the model+cell once,
        // up front, so we never have to swap the field in or out.
        let enter_model = model.clone();
        let enter_cell = Rc::clone(&text_cell);
        let field = TextField::new(font.clone())
            .with_text_cell(Rc::clone(&text_cell))
            .with_font_size(20.0)
            .with_padding(10.0)
            .with_char_filter(|c| {
                c.is_ascii_digit() || c == 'x' || c == 'X' || c.is_ascii_hexdigit()
            })
            .on_enter(move |_| {
                let input = enter_cell.borrow().clone();
                if enter_model
                    .borrow_mut()
                    .commit_play_deal_dialog(&input)
                    .is_ok()
                {
                    enter_cell.borrow_mut().clear();
                }
                agg_gui::animation::request_draw();
            });
        Self {
            bounds: Rect::default(),
            children: vec![Box::new(field)],
            model,
            font,
            hover: None,
            error: None,
            text_cell,
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
                self.text_cell.borrow_mut().clear();
                self.model.borrow_mut().cancel_play_deal_dialog();
            }
            Button::Play => self.commit(),
        }
        agg_gui::animation::request_draw();
    }

    /// Read the current text, hand it to AppModel, surface any
    /// validation error inline. Triggered by clicking Play and by
    /// the TextField's Enter handler (set up in `paint` once per
    /// open).
    fn commit(&mut self) {
        let input = self.text_cell.borrow().clone();
        match self.model.borrow_mut().commit_play_deal_dialog(&input) {
            Ok(()) => {
                self.error = None;
                self.text_cell.borrow_mut().clear();
            }
            Err(msg) => {
                self.error = Some(msg);
            }
        }
    }

    /// Reset transient state on the open transition. Sends
    /// FocusGained to the TextField so the cursor blinks
    /// immediately and typed keystrokes have somewhere to land.
    /// The text itself is cleared via `text_cell` — TextField
    /// picks the cleared value up on its next `layout` (which
    /// calls `sync_from_text_cell`).
    fn on_open(&mut self) {
        self.text_cell.borrow_mut().clear();
        self.error = None;
        self.hover = None;
        if let Some(field) = self.children.get_mut(0) {
            field.on_event(&Event::FocusGained);
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
        // Resync the TextField's bounds to the current field rect
        // every layout pass — the dialog repositions when the host
        // window resizes.
        if let Some((fx, fy, fw, fh)) = self.field_rect() {
            if let Some(child) = self.children.get_mut(0) {
                child.set_bounds(Rect::new(fx, fy, fw, fh));
                child.layout(Size::new(fw, fh));
            }
        }
        available
    }

    fn is_visible(&self) -> bool {
        self.is_open()
    }

    fn has_active_modal(&self) -> bool {
        self.is_open()
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        // Open-transition: clear input, refresh callbacks, focus.
        let open = self.is_open();
        if open && !self.last_open {
            self.on_open();
        }
        self.last_open = open;
        if !open {
            return;
        }
        // Make sure TextField bounds are current (resize while open).
        if let Some((fx, fy, fw, fh)) = self.field_rect() {
            if let Some(child) = self.children.get_mut(0) {
                child.set_bounds(Rect::new(fx, fy, fw, fh));
            }
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

        // TextField paints itself via the framework's child walk
        // after this `paint` returns — see widget/paint.rs.

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
                // Forward clicks inside the field rect to the
                // TextField so the user can position the cursor
                // and drag-select. The modal-path routing stops at
                // this dialog, so children don't receive mouse
                // events automatically.
                if let Some(field_r) = self.field_rect() {
                    if point_in(field_r, pos.x, pos.y) {
                        if let Some(child) = self.children.get_mut(0) {
                            let local = Point::new(pos.x - field_r.0, pos.y - field_r.1);
                            let translated = Event::MouseDown {
                                pos: local,
                                button: MouseButton::Left,
                                modifiers: Default::default(),
                            };
                            child.on_event(&translated);
                            // Make sure the field is focused — it
                            // gets cursor blink + key forwarding
                            // from us once focused.
                            child.on_event(&Event::FocusGained);
                        }
                        return EventResult::Consumed;
                    }
                }
                if let Some(button) = self.hit_button(pos.x, pos.y) {
                    self.click_button(button);
                }
                EventResult::Consumed
            }
            Event::MouseMove { pos } => {
                // Hover for buttons.
                let hover = self.hit_button(pos.x, pos.y);
                if hover != self.hover {
                    self.hover = hover;
                    agg_gui::animation::request_draw();
                }
                // Forward mouse-move to TextField for drag-select.
                if let Some(field_r) = self.field_rect() {
                    if let Some(child) = self.children.get_mut(0) {
                        let local = Point::new(pos.x - field_r.0, pos.y - field_r.1);
                        child.on_event(&Event::MouseMove { pos: local });
                    }
                }
                EventResult::Consumed
            }
            Event::MouseUp {
                button: MouseButton::Left,
                pos,
                modifiers,
            } => {
                if let Some(field_r) = self.field_rect() {
                    if let Some(child) = self.children.get_mut(0) {
                        let local = Point::new(pos.x - field_r.0, pos.y - field_r.1);
                        child.on_event(&Event::MouseUp {
                            pos: local,
                            button: MouseButton::Left,
                            modifiers: *modifiers,
                        });
                    }
                }
                EventResult::Consumed
            }
            Event::KeyDown { key, .. } => {
                // Forward keystroke to the TextField; if it
                // returns Ignored (Escape with no selection,
                // unhandled key), fall through to the dialog's
                // own handlers (Cancel on Escape).
                let child_result = if let Some(child) = self.children.get_mut(0) {
                    child.on_event(event)
                } else {
                    EventResult::Ignored
                };
                if child_result == EventResult::Consumed {
                    self.error = None;
                    agg_gui::animation::request_draw();
                    return EventResult::Consumed;
                }
                match key {
                    Key::Escape => self.click_button(Button::Cancel),
                    Key::Enter => self.click_button(Button::Play),
                    _ => {}
                }
                EventResult::Consumed
            }
            _ => EventResult::Consumed,
        }
    }
}
