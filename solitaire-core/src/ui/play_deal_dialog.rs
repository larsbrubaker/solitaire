//! "Play deal number…" modal — accepts a decimal Microsoft FreeCell
//! game number or a `u64` seed (decimal / 0x-prefixed hex) for the
//! other variants, and starts a new deal at that number.
//!
//! The numeric input is an [`agg_gui::widgets::TextField`] (with a
//! `char_filter` restricting input to decimal digits, hex letters,
//! and the `x`/`X` separator used by 0x-prefixed seeds).
//!
//! Event routing follows the same pattern as `HelpDialog`: we do NOT
//! override `has_active_modal`, so the framework's hit-testing
//! descends naturally into the TextField child for clicks in its
//! bounds (the click sets focus, and subsequent keystrokes route
//! via the focus path). The dialog still hosts the scrim, panel,
//! title/hint copy, Cancel/Play buttons, and the inline error
//! line. Escape dismissal + first-frame focus arrival use
//! `on_unconsumed_key` so the field receives keystrokes immediately
//! on open, before any click sets a "real" App-level focus.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, Modifiers, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::{TextField, TextFieldTheme};

use super::app_model::SharedModel;

const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x90);
const PANEL_BG: Color = Color::from_rgb8(0x1a, 0x2c, 0x20);
const PANEL_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const TITLE_TEXT: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BODY_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const HINT_TEXT: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0xb0);
const ERROR_TEXT: Color = Color::from_rgb8(0xff, 0x80, 0x80);
/// Darker shade than `PANEL_BG` so the field reads as carved into
/// the panel rather than sitting on top of it.
const FIELD_BG: Color = Color::from_rgb8(0x0c, 0x1c, 0x12);
const FIELD_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
/// Focused state — same accent yellow as the dialog title so the
/// glow ties the active field back to the dialog brand.
const FIELD_BORDER_FOCUSED: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);

/// Match the dialog's dark-green palette. The TextField reads from
/// `theme` instead of the ambient `visuals()`, so the field sits
/// flush in the panel instead of contrasting against it with the
/// agg-gui demo theme's lighter widget background.
fn field_theme() -> TextFieldTheme {
    TextFieldTheme {
        background: Some(FIELD_BG),
        text_color: Some(BODY_TEXT),
        placeholder_color: Some(HINT_TEXT),
        border_color: Some(FIELD_BORDER),
        border_color_hovered: Some(FIELD_BORDER_FOCUSED),
        border_color_focused: Some(FIELD_BORDER_FOCUSED),
        cursor_color: Some(FIELD_BORDER_FOCUSED),
        // Selection / unfocused-selection use a translucent gold so
        // dragging across digits keeps the dialog's accent visible.
        selection_bg: Some(Color::from_rgba8(0xff, 0xd7, 0x00, 0x55)),
        selection_bg_unfocused: Some(Color::from_rgba8(0xff, 0xd7, 0x00, 0x33)),
        border_radius: Some(6.0),
    }
}

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
    /// Backing buffer for the TextField. Owned by the dialog so we
    /// can clear it on open / commit without taking a mutable
    /// borrow on the child widget.
    text_cell: Rc<RefCell<String>>,
    last_open: bool,
}

impl PlayDealDialog {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let text_cell = Rc::new(RefCell::new(String::new()));
        // `on_enter` captures the model+cell once, up front, so we
        // never rebuild the field. The TextField is at index 0 in
        // `children` for the lifetime of the dialog.
        let enter_model = model.clone();
        let enter_cell = Rc::clone(&text_cell);
        let field = TextField::new(font.clone())
            .with_text_cell(Rc::clone(&text_cell))
            .with_font_size(20.0)
            .with_padding(10.0)
            .with_theme(field_theme())
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
    /// immediately and `on_unconsumed_key` has a focused field to
    /// forward keystrokes into. The TextField also synchronises
    /// its internal text from `text_cell` on its next `layout`
    /// pass — we cleared the cell here so re-opening the dialog
    /// after a previous commit shows a blank field.
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

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let open = self.is_open();
        if open && !self.last_open {
            self.on_open();
        }
        self.last_open = open;
        if !open {
            return;
        }
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

        // The TextField paints itself via the framework's child
        // walk after this `paint` returns.

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
                // Mouse-down reaches us only when the click missed
                // the TextField (framework descends into children
                // first via hit-testing). Either it hit a button or
                // it hit empty panel / scrim chrome — either way we
                // consume it so it doesn't leak to the playfield.
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
            // Swallow every other event so the playfield underneath
            // doesn't receive stray clicks/scrolls while the dialog
            // is up. Keys are handled in `on_unconsumed_key` below.
            _ => EventResult::Consumed,
        }
    }

    fn on_unconsumed_key(&mut self, key: &Key, modifiers: Modifiers) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        // Forward to the TextField first. It returns Consumed for
        // characters / Backspace / arrow keys / clipboard
        // shortcuts and Ignored for things it doesn't claim
        // (notably Escape with no selection, per the agg-gui
        // change we landed alongside this dialog).
        let event = Event::KeyDown {
            key: key.clone(),
            modifiers,
        };
        let child_result = if let Some(child) = self.children.get_mut(0) {
            child.on_event(&event)
        } else {
            EventResult::Ignored
        };
        if child_result == EventResult::Consumed {
            self.error = None;
            agg_gui::animation::request_draw();
            return EventResult::Consumed;
        }
        match key {
            Key::Escape => {
                self.click_button(Button::Cancel);
                EventResult::Consumed
            }
            Key::Enter => {
                self.click_button(Button::Play);
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app_model::shared_model;
    use agg_gui::event::Modifiers;
    use agg_gui::text::Font;

    fn test_font() -> Arc<Font> {
        const FONT_BYTES: &[u8] = include_bytes!("../../assets/CascadiaCode.ttf");
        Arc::new(Font::from_slice(FONT_BYTES).unwrap())
    }

    /// Sanity check that a standalone TextField (no dialog wrapping)
    /// accepts keystrokes after FocusGained. If this passes but the
    /// dialog test fails, the bug is in dialog forwarding, not in
    /// TextField itself.
    #[test]
    fn standalone_text_field_accepts_typing() {
        use agg_gui::widgets::TextField;
        let mut field = TextField::new(test_font()).with_font_size(13.0);
        field.set_bounds(Rect::new(0.0, 0.0, 200.0, 32.0));
        field.layout(Size::new(200.0, 32.0));
        field.on_event(&Event::FocusGained);
        for c in ['a', 'b', 'c'] {
            field.on_event(&Event::KeyDown {
                key: Key::Char(c),
                modifiers: Modifiers::default(),
            });
        }
        assert_eq!(field.text(), "abc");
    }

    /// Open the dialog, fake the open-transition + first paint
    /// (which calls on_open and sends FocusGained to the
    /// TextField), then send digit keystrokes through the dialog's
    /// `on_unconsumed_key` — the same path the App's event loop
    /// uses when no focus path is registered yet. Confirm the
    /// TextField's bound `text_cell` updates.
    #[test]
    fn typing_via_unconsumed_key_updates_text_cell() {
        let model = shared_model();
        // open_play_deal_dialog gates on session.is_some(); for
        // the unit test flip the flag directly.
        model.borrow_mut().play_deal_dialog_open = true;
        let mut dialog = PlayDealDialog::new(model.clone(), test_font());
        dialog.set_bounds(Rect::new(0.0, 0.0, 800.0, 600.0));
        dialog.layout(Size::new(800.0, 600.0));
        // Simulate the open-transition that paint() would run.
        dialog.children[0].on_event(&Event::FocusGained);

        for c in ['1', '2', '3'] {
            dialog.on_unconsumed_key(&Key::Char(c), Modifiers::default());
        }
        assert_eq!(*dialog.text_cell.borrow(), "123");
    }

    /// Escape should bubble out of the TextField (no selection),
    /// trigger the dialog's Cancel handler, and clear the open
    /// flag on the AppModel.
    #[test]
    fn escape_via_unconsumed_key_cancels_dialog() {
        let model = shared_model();
        model.borrow_mut().play_deal_dialog_open = true;
        let mut dialog = PlayDealDialog::new(model.clone(), test_font());
        dialog.set_bounds(Rect::new(0.0, 0.0, 800.0, 600.0));
        dialog.layout(Size::new(800.0, 600.0));
        dialog.children[0].on_event(&Event::FocusGained);

        let r = dialog.on_unconsumed_key(&Key::Escape, Modifiers::default());
        assert_eq!(r, EventResult::Consumed);
        assert!(!model.borrow().play_deal_dialog_open);
    }
}
