//! Shared Solitaire widget tree and UI builders.
//!
//! Native and WASM shells must build the game through this module instead of
//! constructing widgets directly. Platform crates own only OS/browser wiring;
//! every game screen, menu, layout, and widget tree lives here.

use std::sync::Arc;

use agg_gui::text::Font;
use agg_gui::widgets::Window as AggWindow;
use agg_gui::{App, PerformanceView};

pub mod animation;
pub mod app_model;
pub mod app_root;
pub mod confirm_dialog;
pub mod dyn_session;
pub mod game_widget;
pub mod help_content;
pub mod help_widget;
pub mod hud_widget;
pub mod icons;
pub mod layout;
pub mod menu_widget;
pub mod overlay_stack;
pub mod play_deal_dialog;
pub mod seed_gen_window;
pub mod title_widget;
pub mod toast;

use app_model::{shared_model, SharedModel};
use app_root::AppRootWidget;
use confirm_dialog::ConfirmDialog;
use game_widget::GameWidget;
use play_deal_dialog::PlayDealDialog;
use help_widget::HelpDialog;
use hud_widget::HudWidget;
use menu_widget::{MenuBarHost, SidebarMenuHost};
use overlay_stack::OverlayStack;
use title_widget::TitleWidget;

use crate::render::CardSpriteAtlas;

/// CascadiaCode bundled into the binary.
const FONT_BYTES: &[u8] = include_bytes!("../../assets/CascadiaCode.ttf");
/// Font Awesome (private-use glyphs only, no Latin coverage) bundled
/// in. Loaded as a separate [`Font`] so widgets can fall back to it
/// when rendering icon code points from [`crate::ui::icons`].
const FA_FONT_BYTES: &[u8] = include_bytes!("../../assets/fa.ttf");

fn load_default_font() -> Arc<Font> {
    Arc::new(Font::from_slice(FONT_BYTES).expect("solitaire default font"))
}

/// Load the Font Awesome icon font. Stays separate from the text
/// font because CascadiaCode doesn't carry FA's private-use code
/// points — call sites that paint an icon glyph (e.g. on a button)
/// must temporarily switch [`DrawCtx::set_font`] to this one.
pub fn load_fa_font() -> Arc<Font> {
    Arc::new(Font::from_slice(FA_FONT_BYTES).expect("solitaire FA font"))
}

/// Build the shared Solitaire application. Returns the [`App`] hosting the
/// widget tree (title screen + game widget + HUD, switched via the
/// shared `AppModel`) and a clone of the [`SharedModel`] so the
/// platform shell can push frame timings, drain pending URLs, etc.
pub fn build_solitaire_app() -> (App, SharedModel) {
    let model = shared_model();
    let font = load_default_font();
    let fa_font = load_fa_font();
    // Seed an empty stand-in atlas at default card dimensions. The
    // first `GameWidget::paint` after a session starts replaces it with
    // one matching the active variant's actual screen-space card size.
    let atlas = CardSpriteAtlas::build(&font, 90.0, 126.0, 1.0);

    let title = TitleWidget::new(model.clone(), font.clone());
    let game = GameWidget::new(model.clone(), font.clone(), atlas);
    let hud = HudWidget::new(model.clone(), font.clone(), fa_font.clone());
    let menu = MenuBarHost::new(model.clone(), font.clone());
    let sidebar_menu = SidebarMenuHost::new(model.clone(), font.clone());
    let help = HelpDialog::new(model.clone(), font.clone());
    let confirm = ConfirmDialog::new(model.clone(), font.clone());
    let play_deal = PlayDealDialog::new(model.clone(), font.clone(), fa_font.clone());
    let perf_window = build_performance_window(&model, font.clone());
    let seed_gen_window = build_seed_gen_window(&model, font.clone());
    let root = AppRootWidget::new(model.clone());

    // Painted bottom→top, hit-tested top→bottom. Confirmation and help
    // overlays sit at the very top so their scrims cover the menu bar and
    // the title screen chrome equally. Only ONE of `menu` / `sidebar_menu`
    // is visible at a time (gated by chrome mode); both registered so the
    // swap is automatic when the viewport changes. The Performance window
    // lives just under HelpDialog so its title bar is reachable above the
    // menu / HUD but modal overlays still scrim over it.
    //
    // `title` MUST sit below the two menu hosts: `TitleWidget`'s
    // default `hit_test` claims its full bounds (the entire viewport),
    // so if it were stacked above `menu`/`sidebar_menu` the menu's
    // top-strip clicks would be swallowed before they reached
    // `MenuBarHost`.  With the menu on top, its narrow `hit_test`
    // (only the 26 px menu strip) lets clicks on title-screen buttons
    // fall through to the title widget below.
    let stack = OverlayStack::new()
        .add(Box::new(root))
        .add(Box::new(game))
        .add(Box::new(title))
        .add(Box::new(hud))
        .add(Box::new(menu))
        .add(Box::new(sidebar_menu))
        .add(Box::new(perf_window))
        .add(Box::new(seed_gen_window))
        .add(Box::new(help))
        .add(Box::new(confirm))
        .add(Box::new(play_deal));

    (App::new(Box::new(stack)), model)
}

/// Construct the floating window that wraps `agg_gui::PerformanceView`.
///
/// Visibility, position, and size are all wired into shared cells on
/// the [`AppModel`] so persistence rides on the same write-on-change
/// path that the Options menu uses for Klondike / Spider settings:
///
/// * `with_visible_cell` — the close × button writes through to the
///   same cell the Debug menu's "Performance Window" toggle reads.
/// * `with_position_cell` — every layout pass writes the live bounds
///   back into `AppModel.perf_window_bounds`, which `AppRootWidget`
///   diffs against the last-saved value to trigger a settings write
///   when the user moves / resizes the window.
///
/// `with_live_redraw` is intentionally **off**: forcing a redraw every
/// frame the window is visible defeats the reactive event loop the
/// shells run for battery-friendly idle. `with_history_redraw(true)`
/// is narrower: it asks for one redraw only when a new timing sample
/// has been pushed and the graph has not painted that revision yet.
fn build_performance_window(model: &SharedModel, font: Arc<Font>) -> AggWindow {
    let (visible_cell, position_cell, history, saved_bounds) = {
        let m = model.borrow();
        (
            m.show_performance_window.clone(),
            m.perf_window_bounds.clone(),
            m.frame_history.clone(),
            m.perf_window_bounds.get(),
        )
    };
    let view = PerformanceView::new(font.clone(), history)
        .with_padding(12.0)
        .with_sparkline_height(80.0)
        .with_history_redraw(true);
    AggWindow::new("Performance", font, Box::new(view))
        .with_bounds(saved_bounds)
        .with_visible_cell(visible_cell)
        .with_position_cell(position_cell)
        .with_resizable(true)
}

fn build_seed_gen_window(model: &SharedModel, font: Arc<Font>) -> AggWindow {
    let visible = model.borrow().show_seed_gen_window.clone();
    let view = seed_gen_window::SeedGenView::new(font.clone());
    AggWindow::new("Generate Seed Games", font, Box::new(view))
        .with_bounds(agg_gui::geometry::Rect::new(80.0, 80.0, 720.0, 360.0))
        .with_visible_cell(visible)
        .with_resizable(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agg_gui::geometry::{Point, Size};
    use agg_gui::widget::{hit_test_subtree, Widget};

    /// Walk a hit-test path back to its leaf and collect the
    /// `type_name`s of every widget visited (root → leaf).  Used by the
    /// regression test below to verify which sub-tree actually receives
    /// a click on the title screen.
    fn type_chain_for_path(root: &dyn Widget, path: &[usize]) -> Vec<&'static str> {
        let mut chain = vec![root.type_name()];
        let mut current = root;
        for &idx in path {
            let child = current.children()[idx].as_ref();
            chain.push(child.type_name());
            current = child;
        }
        chain
    }

    /// Regression: clicking inside the menu strip on the **title**
    /// screen must route to `MenuBarHost`, not to `TitleWidget`.
    /// `TitleWidget` paints behind the menu and (deliberately) claims
    /// its full bounds for hit-testing of the game-selection buttons,
    /// so the overlay stack must place the menu above it for clicks in
    /// the top 26 px strip to reach the menu bar.
    #[test]
    fn menu_strip_click_on_title_screen_hits_menu_bar() {
        let (mut app, _model) = build_solitaire_app();
        // Desktop-shaped viewport so chrome layout picks `Standard`
        // (top-aligned menu bar, not the sidebar variant). Aspect
        // matters: `chrome::compute` switches to `Sidebar` when
        // `w > h*1.5 && h < 900`, so 1024x768 keeps us in Standard.
        let viewport = Size::new(1024.0, 768.0);
        app.layout(viewport);

        // Menu bar lives in the top 26 px (Y-up: y close to viewport
        // height).  Pick a point safely inside the strip and away from
        // the corners so we exercise an active hit region.
        let click = Point::new(200.0, viewport.height - 8.0);
        let path = hit_test_subtree(app.root(), click)
            .expect("click inside the menu strip must hit something");
        let chain = type_chain_for_path(app.root(), &path);

        assert!(
            chain.contains(&"MenuBarHost"),
            "menu-strip click must reach MenuBarHost, but landed on {chain:?}"
        );
        assert!(
            !chain.contains(&"TitleWidget"),
            "menu-strip click must NOT be swallowed by TitleWidget, got {chain:?}"
        );
    }

    /// Companion check: clicking the title-screen body (well below the
    /// menu strip) still reaches `TitleWidget`, so the buttons remain
    /// usable from the title screen.
    #[test]
    fn title_body_click_still_hits_title_widget() {
        let (mut app, _model) = build_solitaire_app();
        let viewport = Size::new(1024.0, 768.0);
        app.layout(viewport);

        // Mid-viewport, clear of the 26 px menu strip at the top and
        // any HUD strip at the bottom (HUD is hidden on Title anyway).
        let click = Point::new(viewport.width * 0.5, viewport.height * 0.5);
        let path =
            hit_test_subtree(app.root(), click).expect("title-body click must hit something");
        let chain = type_chain_for_path(app.root(), &path);

        assert!(
            chain.contains(&"TitleWidget"),
            "title-body click must reach TitleWidget, got {chain:?}"
        );
    }
}
