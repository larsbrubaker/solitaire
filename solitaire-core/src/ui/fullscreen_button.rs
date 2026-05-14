//! Always-visible fullscreen toggle button.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

const BTN_W: f64 = 112.0;
const BTN_H: f64 = 24.0;
const INSET: f64 = 6.0;

const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);
const TXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

pub struct FullscreenButton {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    font: Arc<Font>,
    hovered: bool,
}

impl FullscreenButton {
    pub fn new(font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            font,
            hovered: false,
        }
    }

    fn rect(&self) -> (f64, f64, f64, f64) {
        let x = self.bounds.x + self.bounds.width - BTN_W - INSET;
        let y = self.bounds.y + self.bounds.height - BTN_H - INSET;
        (x, y, BTN_W, BTN_H)
    }

    fn contains(&self, p: Point) -> bool {
        let (x, y, w, h) = self.rect();
        p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h
    }
}

impl Widget for FullscreenButton {
    fn type_name(&self) -> &'static str {
        "FullscreenButton"
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

    fn hit_test(&self, local_pos: Point) -> bool {
        self.contains(local_pos)
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let (x, y, w, h) = self.rect();
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 6.0);
        ctx.set_fill_color(if self.hovered { BTN_BG_HOVER } else { BTN_BG });
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 6.0);
        ctx.set_stroke_color(BTN_BORDER);
        ctx.set_line_width(1.0);
        ctx.stroke();

        let label = "Full Screen";
        ctx.set_fill_color(TXT);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(14.0);
        if let Some(m) = ctx.measure_text(label) {
            ctx.fill_text(label, x + (w - m.width) / 2.0, y + m.centered_baseline_y(h));
        }
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } if self.contains(*pos) => {
                crate::platform::request_toggle_fullscreen();
                EventResult::Consumed
            }
            Event::MouseMove { pos } => {
                let hovered = self.contains(*pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    agg_gui::animation::request_draw();
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
