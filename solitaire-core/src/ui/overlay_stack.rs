//! Z-stacking container — every child sits at the full bounds; visibility
//! is each child's own concern via `Widget::is_visible`.

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Rect, Size};
use agg_gui::widget::Widget;

pub struct OverlayStack {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
}

impl OverlayStack {
    pub fn new() -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, child: Box<dyn Widget>) -> Self {
        self.children.push(child);
        self
    }
}

impl Default for OverlayStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for OverlayStack {
    fn type_name(&self) -> &'static str {
        "OverlayStack"
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
        for child in self.children.iter_mut() {
            child.set_bounds(Rect::new(0.0, 0.0, available.width, available.height));
            child.layout(available);
        }
        available
    }

    fn paint(&mut self, _ctx: &mut dyn DrawCtx) {
        // No own content; children paint themselves.
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        self.children.iter().any(|c| c.needs_draw())
    }
}
