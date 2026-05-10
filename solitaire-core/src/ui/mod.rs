//! Shared Solitaire widget tree and UI builders.
//!
//! Native and WASM shells must build the game through this module instead of
//! constructing widgets directly. Platform crates own only OS/browser wiring;
//! every game screen, menu, layout, and widget tree lives here.

use agg_gui::App;

mod placeholder_widget;

use placeholder_widget::PlaceholderWidget;

/// Phase 0 stub builder. Returns an [`App`] hosting a single felt-coloured
/// placeholder widget so the native and wasm shells have something to paint.
/// Phase 1 replaces this with the title screen + four `GameWidget<R>` overlays.
pub fn build_solitaire_app() -> App {
    App::new(Box::new(PlaceholderWidget::new()))
}
