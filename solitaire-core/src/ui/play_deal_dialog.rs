//! "Play deal number…" modal — accepts a decimal Microsoft FreeCell
//! game number or a `u64` seed (decimal / 0x-prefixed hex) for the
//! other variants, and starts a new deal at that number.
//!
//! Uses real `agg_gui::widgets::TextField` + `Button` widgets, themed
//! to the dialog's dark-green panel via `TextFieldTheme` /
//! `ButtonTheme`. The buttons carry Font-Awesome icons on the left
//! of the label (X for Cancel, ▶ for Play).
//!
//! Event routing follows the same pattern as `HelpDialog`: we do NOT
//! override `has_active_modal`, so the framework's hit-testing
//! descends naturally into the TextField/Button children. The
//! dialog still hosts the scrim, panel, title/hint copy, and the
//! inline error line. Escape dismissal + first-frame focus arrival
//! use `on_unconsumed_key` so the field receives keystrokes
//! immediately on open.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, Modifiers};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::{Button, ButtonTheme, TextField, TextFieldTheme};

use super::app_model::SharedModel;
use super::icons::{FA_PLAY, FA_XMARK};

const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x90);
const PANEL_BG: Color = Color::from_rgb8(0x1a, 0x2c, 0x20);
const PANEL_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const TITLE_TEXT: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BODY_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const HINT_TEXT: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0xb0);
const ERROR_TEXT: Color = Color::from_rgb8(0xff, 0x80, 0x80);
const FIELD_BG: Color = Color::from_rgb8(0x0c, 0x1c, 0x12);
const FIELD_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
const FIELD_BORDER_FOCUSED: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BG_PRESSED: Color = Color::from_rgb8(0x18, 0x3d, 0x24);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);

const PANEL_W: f64 = 460.0;
const PANEL_H: f64 = 230.0;
const PAD: f64 = 22.0;
const BTN_H: f64 = 40.0;
const BTN_GAP: f64 = 16.0;
const FIELD_H: f64 = 40.0;

fn field_theme() -> TextFieldTheme {
    TextFieldTheme {
        background: Some(FIELD_BG),
        text_color: Some(BODY_TEXT),
        placeholder_color: Some(HINT_TEXT),
        border_color: Some(FIELD_BORDER),
        border_color_hovered: Some(FIELD_BORDER_FOCUSED),
        border_color_focused: Some(FIELD_BORDER_FOCUSED),
        cursor_color: Some(FIELD_BORDER_FOCUSED),
        selection_bg: Some(Color::from_rgba8(0xff, 0xd7, 0x00, 0x55)),
        selection_bg_unfocused: Some(Color::from_rgba8(0xff, 0xd7, 0x00, 0x33)),
        border_radius: Some(6.0),
    }
}

/// Shared Cancel / Play button theme — matches the dialog's dark
/// green chrome with a faint white border that lights up on hover.
fn button_theme() -> ButtonTheme {
    ButtonTheme {
        background: BTN_BG,
        background_hovered: BTN_BG_HOVER,
        background_pressed: BTN_BG_PRESSED,
        label_color: BODY_TEXT,
        border_radius: 8.0,
        focus_ring_color: BTN_BORDER,
        focus_ring_width: 1.5,
    }
}

// Indices of the widget children in `self.children`. Reserved up
// front so the layout/paint helpers don't go out of sync with the
// constructor.
const IDX_FIELD: usize = 0;
const IDX_CANCEL: usize = 1;
const IDX_PLAY: usize = 2;

pub struct PlayDealDialog {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    error: Option<&'static str>,
    text_cell: Rc<RefCell<String>>,
    last_open: bool,
}

impl PlayDealDialog {
    pub fn new(model: SharedModel, font: Arc<Font>, fa_font: Arc<Font>) -> Self {
        let text_cell = Rc::new(RefCell::new(String::new()));

        // ── TextField ────────────────────────────────────────────
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

        // ── Cancel button ────────────────────────────────────────
        let cancel_model = model.clone();
        let cancel_cell = Rc::clone(&text_cell);
        let cancel_btn = Button::new("Cancel", font.clone())
            .with_font_size(17.0)
            .with_theme(button_theme())
            .with_icon(FA_XMARK, fa_font.clone())
            .on_click(move || {
                cancel_cell.borrow_mut().clear();
                cancel_model.borrow_mut().cancel_play_deal_dialog();
                agg_gui::animation::request_draw();
            });

        // ── Play button ──────────────────────────────────────────
        let play_model = model.clone();
        let play_cell = Rc::clone(&text_cell);
        let play_btn = Button::new("Play", font.clone())
            .with_font_size(17.0)
            .with_theme(button_theme())
            .with_icon(FA_PLAY, fa_font.clone())
            .on_click(move || {
                let input = play_cell.borrow().clone();
                if play_model
                    .borrow_mut()
                    .commit_play_deal_dialog(&input)
                    .is_ok()
                {
                    play_cell.borrow_mut().clear();
                }
                agg_gui::animation::request_draw();
            });

        Self {
            bounds: Rect::default(),
            children: vec![Box::new(field), Box::new(cancel_btn), Box::new(play_btn)],
            model,
            font,
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

    fn field_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (px, py, pw, _) = self.panel_rect()?;
        Some((px + PAD, py + BTN_H + PAD * 2.0, pw - PAD * 2.0, FIELD_H))
    }

    /// Position the Cancel + Play buttons side-by-side at the
    /// bottom of the panel. Each button advertises its own natural
    /// width via `Button::layout`, so we ask for it and centre the
    /// pair as a group with `BTN_GAP` between them.
    fn layout_buttons(&mut self, available: Size) {
        let Some((px, py, pw, _)) = self.panel_rect() else {
            return;
        };
        let _ = available;
        let y = py + PAD;
        // Ask each button for its preferred size at a generous
        // width budget; Button reports a "fit" width based on its
        // label + icon.
        let probe = Size::new(pw, BTN_H);
        let cancel_size = self.children[IDX_CANCEL].layout(probe);
        let play_size = self.children[IDX_PLAY].layout(probe);
        let total_w = cancel_size.width + play_size.width + BTN_GAP;
        let start_x = px + (pw - total_w) / 2.0;
        self.children[IDX_CANCEL].set_bounds(Rect::new(
            start_x,
            y,
            cancel_size.width,
            cancel_size.height,
        ));
        let play_x = start_x + cancel_size.width + BTN_GAP;
        self.children[IDX_PLAY].set_bounds(Rect::new(play_x, y, play_size.width, play_size.height));
    }

    /// Reset transient state on the open transition. Sends
    /// FocusGained to the TextField so the cursor blinks
    /// immediately and `on_unconsumed_key` has a focused field to
    /// forward keystrokes into.
    fn on_open(&mut self) {
        self.text_cell.borrow_mut().clear();
        self.error = None;
        self.children[IDX_FIELD].on_event(&Event::FocusGained);
    }
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
            self.children[IDX_FIELD].set_bounds(Rect::new(fx, fy, fw, fh));
            self.children[IDX_FIELD].layout(Size::new(fw, fh));
        }
        self.layout_buttons(available);
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
        // Re-layout in case the host window resized while open.
        if let Some((fx, fy, fw, fh)) = self.field_rect() {
            self.children[IDX_FIELD].set_bounds(Rect::new(fx, fy, fw, fh));
        }
        self.layout_buttons(Size::new(self.bounds.width, self.bounds.height));

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

        if let Some(err) = self.error {
            ctx.set_fill_color(ERROR_TEXT);
            ctx.set_font_size(13.0);
            if let Some(m) = ctx.measure_text(err) {
                let (_, fy, _, _) = self.field_rect().unwrap_or((0.0, 0.0, 0.0, 0.0));
                ctx.fill_text(err, px + (pw - m.width) / 2.0, fy - 20.0);
            }
        }

        // TextField + the two Buttons paint themselves through the
        // framework's child walk.
    }

    fn hit_test(&self, _local_pos: Point) -> bool {
        self.is_visible()
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        // The TextField + Buttons receive their own mouse events
        // via the framework's hit-test descent. We only see events
        // that missed every child (panel chrome, scrim). Consume so
        // they don't leak to the playfield below.
        let _ = event;
        EventResult::Consumed
    }

    fn on_unconsumed_key(&mut self, key: &Key, modifiers: Modifiers) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        // Forward to the TextField first. It returns Consumed for
        // characters / Backspace / arrows / clipboard shortcuts and
        // Ignored for things it doesn't claim (Escape with no
        // selection, etc.).
        let event = Event::KeyDown {
            key: key.clone(),
            modifiers,
        };
        let child_result = self.children[IDX_FIELD].on_event(&event);
        if child_result == EventResult::Consumed {
            self.error = None;
            agg_gui::animation::request_draw();
            return EventResult::Consumed;
        }
        match key {
            Key::Escape => {
                self.text_cell.borrow_mut().clear();
                self.model.borrow_mut().cancel_play_deal_dialog();
                agg_gui::animation::request_draw();
                EventResult::Consumed
            }
            Key::Enter => {
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
                agg_gui::animation::request_draw();
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

    fn test_fa_font() -> Arc<Font> {
        const FA_BYTES: &[u8] = include_bytes!("../../assets/fa.ttf");
        Arc::new(Font::from_slice(FA_BYTES).unwrap())
    }

    #[test]
    fn typing_via_unconsumed_key_updates_text_cell() {
        let model = shared_model();
        model.borrow_mut().play_deal_dialog_open = true;
        let mut dialog = PlayDealDialog::new(model.clone(), test_font(), test_fa_font());
        dialog.set_bounds(Rect::new(0.0, 0.0, 800.0, 600.0));
        dialog.layout(Size::new(800.0, 600.0));
        dialog.children[IDX_FIELD].on_event(&Event::FocusGained);

        for c in ['1', '2', '3'] {
            dialog.on_unconsumed_key(&Key::Char(c), Modifiers::default());
        }
        assert_eq!(*dialog.text_cell.borrow(), "123");
    }

    #[test]
    fn escape_via_unconsumed_key_cancels_dialog() {
        let model = shared_model();
        model.borrow_mut().play_deal_dialog_open = true;
        let mut dialog = PlayDealDialog::new(model.clone(), test_font(), test_fa_font());
        dialog.set_bounds(Rect::new(0.0, 0.0, 800.0, 600.0));
        dialog.layout(Size::new(800.0, 600.0));
        dialog.children[IDX_FIELD].on_event(&Event::FocusGained);

        let r = dialog.on_unconsumed_key(&Key::Escape, Modifiers::default());
        assert_eq!(r, EventResult::Consumed);
        assert!(!model.borrow().play_deal_dialog_open);
    }
}
