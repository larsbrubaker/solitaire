//! Help dialog overlay — a modal-feeling markdown panel for About and
//! per-variant rules. Hosts an `agg_gui::widgets::MarkdownView` as a
//! child sized to the panel's content area; clicks inside that area go
//! to the markdown view through the framework's normal hit-testing,
//! clicks outside (or on the close X) close the dialog.
//!
//! The dialog claims the full window via its default `hit_test` so
//! events under the panel don't fall through to the playfield. We
//! deliberately don't override `has_active_modal` — that would mean
//! the framework's modal routing skipped descent into our child, and
//! the user could no longer scroll / click inside the markdown view.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, Modifiers, MouseButton};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::{MarkdownView, ScrollView};

use super::app_model::{HelpKind, SharedModel};
use super::help_content::{markdown_for, title_for};

const PANEL_MAX_W: f64 = 720.0;
const PANEL_MAX_H: f64 = 600.0;
const PANEL_MARGIN: f64 = 32.0;
const TITLE_H: f64 = 40.0;
const PANEL_PAD: f64 = 16.0;
const CORNER_R: f64 = 12.0;
const CLOSE_SIZE: f64 = 26.0;
const CLOSE_INSET: f64 = 8.0;

const SCRIM: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0xa8);
const PANEL_BG: Color = Color::from_rgb8(0x1a, 0x2c, 0x20);
const PANEL_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x40);
const TITLE_TEXT: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const CLOSE_BG: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x14);
const CLOSE_BG_HOVER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x30);
const CLOSE_X: Color = Color::from_rgb8(0xff, 0xff, 0xff);

pub struct HelpDialog {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    /// Tracks which kind the child MarkdownView is currently rendering;
    /// rebuilt whenever the model reports a different `HelpKind`.
    rendered_kind: Option<HelpKind>,
    close_hovered: bool,
}

impl HelpDialog {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            rendered_kind: None,
            close_hovered: false,
        }
    }

    fn current_kind(&self) -> Option<HelpKind> {
        self.model.borrow().help
    }

    /// Panel bounds in widget-local Y-up coords. `None` if the host
    /// hasn't been sized yet.
    fn panel_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if self.bounds.width <= 0.0 || self.bounds.height <= 0.0 {
            return None;
        }
        let panel_w = (self.bounds.width - 2.0 * PANEL_MARGIN).min(PANEL_MAX_W);
        let panel_h = (self.bounds.height - 2.0 * PANEL_MARGIN).min(PANEL_MAX_H);
        let panel_x = (self.bounds.width - panel_w) / 2.0;
        let panel_y = (self.bounds.height - panel_h) / 2.0;
        Some((panel_x, panel_y, panel_w, panel_h))
    }

    fn content_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (px, py, pw, ph) = self.panel_rect()?;
        let cx = px + PANEL_PAD;
        let cw = pw - 2.0 * PANEL_PAD;
        // Y-up: title bar sits at the panel TOP (high Y). MarkdownView
        // fills the rest below it.
        let cy = py + PANEL_PAD;
        let ch = (ph - TITLE_H - 2.0 * PANEL_PAD).max(1.0);
        Some((cx, cy, cw, ch))
    }

    /// Close button rect in widget-local coords (top-right of panel,
    /// inside the title-bar strip).
    fn close_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (px, py, pw, ph) = self.panel_rect()?;
        let x = px + pw - CLOSE_SIZE - CLOSE_INSET;
        let y = py + ph - CLOSE_SIZE - CLOSE_INSET;
        Some((x, y, CLOSE_SIZE, CLOSE_SIZE))
    }

    fn rebuild_markdown_if_needed(&mut self) {
        let kind = self.current_kind();
        if kind == self.rendered_kind {
            return;
        }
        self.children.clear();
        if let Some(k) = kind {
            let md = MarkdownView::new(markdown_for(k), self.font.clone())
                .with_padding(0.0)
                .with_font_size(15.0)
                .on_link_click(crate::platform::request_open_url);
            // Wrap in a ScrollView so help text taller than the panel
            // body stays scrollable. Without this the markdown lays
            // itself out at total content height and overflows the
            // panel — historically painting up into the title strip,
            // which looked like "lines piled on top of each other."
            let scroll = ScrollView::new(Box::new(md));
            self.children.push(Box::new(scroll));
        }
        self.rendered_kind = kind;
        self.update_child_bounds();
    }

    fn update_child_bounds(&mut self) {
        let Some((cx, cy, cw, ch)) = self.content_rect() else {
            return;
        };
        if let Some(child) = self.children.first_mut() {
            let rect = Rect::new(cx, cy, cw, ch);
            child.set_bounds(rect);
            child.layout(Size::new(cw, ch));
            // `ScrollView::layout` resets its own `bounds.x/y` to (0, 0);
            // re-apply so the framework's default child-translate places
            // the scroll panel at our panel's content origin instead of
            // HelpDialog's bottom-left. Without this, the markdown
            // paints far off to the side and we see only the slice that
            // happens to fall inside `clip_children_rect`.
            child.set_bounds(rect);
        }
    }

    fn close(&self) {
        self.model.borrow_mut().help = None;
        agg_gui::animation::request_draw();
    }

    fn point_in(rect: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
        x >= rect.0 && x <= rect.0 + rect.2 && y >= rect.1 && y <= rect.1 + rect.3
    }
}

impl Widget for HelpDialog {
    fn type_name(&self) -> &'static str {
        "HelpDialog"
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
        self.current_kind().is_some()
    }

    /// Constrain the child ScrollView's paint to the panel's content
    /// area. Without this, content that overflows the body's bounds
    /// would paint up into the title strip — which manifested as the
    /// rules text appearing on top of itself near the dialog's top.
    fn clip_children_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.content_rect()
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        self.rebuild_markdown_if_needed();
        let Some((px, py, pw, ph)) = self.panel_rect() else {
            return;
        };
        // Scrim over the whole window.
        ctx.begin_path();
        ctx.rect(0.0, 0.0, self.bounds.width, self.bounds.height);
        ctx.set_fill_color(SCRIM);
        ctx.fill();
        // Panel.
        ctx.begin_path();
        ctx.rounded_rect(px, py, pw, ph, CORNER_R);
        ctx.set_fill_color(PANEL_BG);
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(px, py, pw, ph, CORNER_R);
        ctx.set_stroke_color(PANEL_BORDER);
        ctx.set_line_width(1.5);
        ctx.stroke();
        // Title.
        if let Some(kind) = self.rendered_kind {
            ctx.set_fill_color(TITLE_TEXT);
            ctx.set_font(self.font.clone());
            ctx.set_font_size(20.0);
            let title = title_for(kind);
            // Y-up: title bar lives at the TOP of the panel. Centre the
            // title text vertically within the title strip.
            let title_strip_y = py + ph - TITLE_H;
            if let Some(m) = ctx.measure_text(title) {
                let baseline = title_strip_y + m.centered_baseline_y(TITLE_H);
                ctx.fill_text(title, px + PANEL_PAD, baseline);
            }
        }
        // Close X.
        if let Some(rect) = self.close_rect() {
            let bg = if self.close_hovered {
                CLOSE_BG_HOVER
            } else {
                CLOSE_BG
            };
            ctx.begin_path();
            ctx.rounded_rect(rect.0, rect.1, rect.2, rect.3, 6.0);
            ctx.set_fill_color(bg);
            ctx.fill();
            ctx.set_fill_color(CLOSE_X);
            ctx.set_font_size(20.0);
            // U+00D7 MULTIPLICATION SIGN — universally available, looks
            // like a bold X. Centred within the close button.
            let glyph = "\u{00D7}";
            if let Some(m) = ctx.measure_text(glyph) {
                let lx = rect.0 + (rect.2 - m.width) / 2.0;
                let ly = rect.1 + m.centered_baseline_y(rect.3);
                ctx.fill_text(glyph, lx, ly);
            }
        }
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
                if let Some(close) = self.close_rect() {
                    if Self::point_in(close, pos.x, pos.y) {
                        self.close();
                        return EventResult::Consumed;
                    }
                }
                // Inside the panel (but outside the close X): the
                // framework already gave the MarkdownView a chance via
                // child hit-testing. Reaching us means the click landed
                // in the title-strip — swallow it so it doesn't reach
                // anything below.
                if let Some(panel) = self.panel_rect() {
                    if Self::point_in(panel, pos.x, pos.y) {
                        return EventResult::Consumed;
                    }
                }
                // Outside the panel — dismiss.
                self.close();
                EventResult::Consumed
            }
            Event::MouseMove { pos } => {
                let was = self.close_hovered;
                self.close_hovered = self
                    .close_rect()
                    .is_some_and(|r| Self::point_in(r, pos.x, pos.y));
                if was != self.close_hovered {
                    agg_gui::animation::request_draw();
                }
                // Don't consume — let MarkdownView see hover for selection
                // / link cursors when the move is over its bounds.
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    fn on_unconsumed_key(&mut self, key: &Key, _modifiers: Modifiers) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        if matches!(key, Key::Escape) {
            self.close();
            return EventResult::Consumed;
        }
        EventResult::Ignored
    }

    // No `needs_draw` override — fall through to the default
    // implementation that returns `false` and lets the framework
    // poll children individually (the inner `MarkdownView` /
    // `ScrollView` already call `request_draw()` from their own event
    // handlers when hover, selection, scroll position, or focus
    // change).  Forcing a redraw every frame just because the dialog
    // is open prevented the reactive event loop from going idle.
}
