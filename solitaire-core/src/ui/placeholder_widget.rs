//! Phase 0 placeholder widget — paints a solid felt-green background.
//!
//! Replaced in Phase 1 by `app_root::AppRootWidget` (title screen + four
//! `GameWidget<R>` overlays).

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Rect, Size};
use agg_gui::widget::Widget;

const FELT_GREEN: Color = Color::from_rgb8(0x0c, 0x6b, 0x3a);

pub struct PlaceholderWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
}

impl PlaceholderWidget {
    pub fn new() -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
        }
    }
}

impl Default for PlaceholderWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for PlaceholderWidget {
    fn type_name(&self) -> &'static str {
        "PlaceholderWidget"
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

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let b = self.bounds;
        ctx.set_fill_color(FELT_GREEN);
        ctx.rect(0.0, 0.0, b.width, b.height);
        ctx.fill();
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }
}
