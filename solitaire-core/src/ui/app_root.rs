//! Bottom layer of the widget stack — paints the felt background, the
//! single continuous chrome-strip background across the top, and a
//! "win" overlay if the active session has been won.
//!
//! The chrome strip is painted HERE (rather than by the menu / HUD
//! widgets) for two reasons: this widget paints FIRST (bottom of the
//! `OverlayStack`), so the menu bar and HUD buttons composite on top of
//! one uninterrupted bar; and it is visible on EVERY screen, so the
//! home screen gets the same full-width strip the game screens do
//! instead of a lone floating "Menu" box. The fill uses the theme's
//! `top_bar_bg`, which is exactly what the agg-gui `MenuBar` paints
//! behind itself, so the menu slice blends seamlessly into the strip.

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Rect, Size};
use agg_gui::widget::Widget;

use crate::render::FELT_GREEN;

use super::app_model::SharedModel;

pub struct AppRootWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl AppRootWidget {
    pub fn new(model: SharedModel) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
        }
    }
}

impl Widget for AppRootWidget {
    fn type_name(&self) -> &'static str {
        "AppRootWidget"
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
        ctx.begin_path();
        ctx.rect(0.0, 0.0, b.width, b.height);
        ctx.set_fill_color(FELT_GREEN);
        ctx.fill();

        // ONE continuous chrome strip across the top of the viewport,
        // spanning the menu area + the HUD area. Painted with the same
        // `top_bar_bg` the `MenuBar` fills behind itself so the two
        // never read as mismatched boxes. `layout::compute` derives the
        // strip height per layout (it grows on touch), so this tracks
        // the touch latch automatically.
        let strip = super::layout::compute(Size::new(b.width, b.height)).strip_rect;
        let strip_bg = ctx.visuals().top_bar_bg;
        ctx.begin_path();
        ctx.rect(strip.x, strip.y, strip.width, strip.height);
        ctx.set_fill_color(strip_bg);
        ctx.fill();

        // Persist Performance window state on idle.  This widget paints
        // first (bottom of the OverlayStack) but it runs AFTER the
        // current frame's `App::layout` pass — by which time the
        // agg-gui `Window` has already written its current bounds into
        // `AppModel.perf_window_bounds` and any close-button click
        // has flipped `AppModel.show_performance_window`.  The model
        // diffs against the last-saved snapshot internally, so this
        // is a cheap no-op on every frame except the ones where the
        // user actually moved / resized / closed the window.
        self.model.borrow().maybe_save_perf_window_settings();
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }
}
