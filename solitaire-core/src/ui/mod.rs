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
use menu_widget::MenuBarHost;
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

/// Solitaire-branded visuals — start from agg-gui's `dark` palette
/// and override the chrome / accent colours so the menu bar, popup
/// dropdowns, and any `ctx.visuals()`-driven widget paint in the
/// same dark-green palette as our hand-themed Buttons + TextField.
fn solitaire_visuals() -> agg_gui::theme::Visuals {
    use agg_gui::color::Color;
    let mut v = agg_gui::theme::Visuals::dark();
    // Green felt frame matching `HUD_BG` (~0x095 220x2c with alpha).
    v.top_bar_bg = Color::from_rgb8(0x09, 0x52, 0x2c);
    v.bg_color = Color::from_rgb8(0x06, 0x3a, 0x1f);
    v.panel_fill = Color::from_rgb8(0x12, 0x33, 0x21);
    // Menu popups + dialog windows: dark green panel matching the
    // PANEL_BG used by `play_deal_dialog`.
    v.window_fill = Color::from_rgb8(0x1a, 0x2c, 0x20);
    v.window_title_fill = Color::from_rgb8(0x12, 0x22, 0x18);
    v.window_title_fill_drag = Color::from_rgb8(0x18, 0x2c, 0x20);
    v.window_stroke = Color::from_rgba8(0xff, 0xff, 0xff, 0x40);
    v.window_title_text = Color::from_rgb8(0xff, 0xd7, 0x00);
    // Hover / active accent: brighter green so the highlight reads
    // against the dark felt background. Matches BTN_BG_HOVER.
    let accent = Color::from_rgb8(0x29, 0x68, 0x3e);
    v.accent = accent;
    v.accent_hovered = Color::from_rgb8(0x36, 0x82, 0x4e);
    v.accent_pressed = Color::from_rgb8(0x18, 0x3d, 0x24);
    v.accent_focus = Color::from_rgba8(0x29, 0x68, 0x3e, 0x73);
    v.widget_stroke_active = v.accent_pressed;
    // Subtle widget surface for muted Buttons, ToggleSwitch tracks.
    v.widget_bg = Color::from_rgb8(0x1f, 0x4d, 0x2e);
    v.widget_bg_hovered = Color::from_rgb8(0x29, 0x68, 0x3e);
    v.widget_stroke = Color::from_rgba8(0xff, 0xff, 0xff, 0x55);
    // Selection (text fields, list rows): translucent gold to match
    // the title / hint accent in the play-deal dialog.
    v.selection_bg = Color::from_rgba8(0xff, 0xd7, 0x00, 0x55);
    v.selection_bg_unfocused = Color::from_rgba8(0xff, 0xd7, 0x00, 0x33);
    v
}

/// Build the shared Solitaire application. Returns the [`App`] hosting the
/// widget tree (title screen + game widget + HUD, switched via the
/// shared `AppModel`) and a clone of the [`SharedModel`] so the
/// platform shell can push frame timings, drain pending URLs, etc.
pub fn build_solitaire_app() -> (App, SharedModel) {
    let model = shared_model();
    // Apply the Solitaire dark-green visuals once at startup so
    // every `ctx.visuals()`-driven paint (menu bar, popups,
    // ToggleSwitch, sliders, etc.) sits in the same palette as
    // our hand-themed Buttons + TextField.
    agg_gui::theme::set_visuals(solitaire_visuals());
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
    let help = HelpDialog::new(model.clone(), font.clone());
    let confirm = ConfirmDialog::new(model.clone(), font.clone());
    let play_deal = PlayDealDialog::new(model.clone(), font.clone(), fa_font.clone());
    let perf_window = build_performance_window(&model, font.clone());
    let seed_gen_window = build_seed_gen_window(&model, font.clone());
    let root = AppRootWidget::new(model.clone());

    // Painted bottom→top, hit-tested top→bottom. Confirmation and
    // help overlays sit at the very top so their scrims cover the
    // bottom chrome and the title screen chrome equally. The
    // Performance window lives just under HelpDialog so its title
    // bar is reachable above the menu / HUD but modal overlays
    // still scrim over it.
    //
    // `title` MUST sit below the menu host: `TitleWidget`'s
    // default `hit_test` claims its full bounds (the entire
    // viewport), so if it were stacked above `menu` the menu
    // strip's clicks would be swallowed before they reached
    // `MenuBarHost`. With the menu on top, its narrow `hit_test`
    // (only the 26 px bottom strip) lets clicks on title-screen
    // buttons fall through to the title widget below.
    let stack = OverlayStack::new()
        .add(Box::new(root))
        .add(Box::new(game))
        .add(Box::new(title))
        .add(Box::new(hud))
        .add(Box::new(menu))
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
    /// the bottom 26 px strip to reach the menu bar.
    #[test]
    fn menu_strip_click_on_title_screen_hits_menu_bar() {
        let (mut app, _model) = build_solitaire_app();
        // Desktop-shaped viewport so chrome layout picks `Standard`
        // (bottom-aligned menu bar, not the sidebar variant). Aspect
        // matters: `chrome::compute` switches to `Sidebar` when
        // `w > h*1.5 && h < 900`, so 1024x768 keeps us in Standard.
        let viewport = Size::new(1024.0, 768.0);
        app.layout(viewport);

        // Menu bar lives in the bottom 26 px (Y-up: y close to 0).
        // Pick a point safely inside the strip and away from the
        // corners so we exercise an active hit region.
        let click = Point::new(200.0, 8.0);
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

    /// Regression: a click inside the HUD strip (just above the
    /// bottom menu bar) must reach a Button child so its on_click
    /// fires. After the bottom-bar consolidation, clicks were
    /// landing on the right element but the test pins the path so
    /// future layout shifts don't break it.
    /// Regression: `HudWidget::sync_children` used to compare its
    /// cached action list against `actions_for()` AFTER appending
    /// the hamburger, so the cache key never matched and every
    /// layout call (= every frame) rebuilt the Button children
    /// from scratch. That blew away the captured-pointer path
    /// between MouseDown and MouseUp, so clicks never fired
    /// `on_click`. Lock the cache key by checking that the child
    /// addresses survive a second layout call.
    #[test]
    fn hud_buttons_stable_across_relayout() {
        let (mut app, model) = build_solitaire_app();
        model.borrow_mut().start_game_with_seed(
            crate::games::GameKind::Klondike,
            42,
        );
        let viewport = Size::new(1024.0, 768.0);
        app.layout(viewport);
        // Collect Button pointer identities after the first layout.
        let click = Point::new(viewport.width * 0.5, 50.0);
        let path_a = hit_test_subtree(app.root(), click).unwrap();
        let addr_a = button_address(app.root(), &path_a);
        // Relayout (the render loop calls app.layout every frame).
        app.layout(viewport);
        let path_b = hit_test_subtree(app.root(), click).unwrap();
        let addr_b = button_address(app.root(), &path_b);
        assert_eq!(
            addr_a, addr_b,
            "HUD Button at the same path must survive relayout — \
             rebuilding every frame loses click capture",
        );
    }

    fn button_address(root: &dyn Widget, path: &[usize]) -> usize {
        let mut w: &dyn Widget = root;
        for &i in path {
            w = w.children()[i].as_ref();
        }
        w as *const dyn Widget as *const () as usize
    }

    #[test]
    fn hud_strip_click_hits_a_button() {
        let (mut app, model) = build_solitaire_app();
        // Start a game so the HUD is visible.
        model.borrow_mut().start_game_with_seed(
            crate::games::GameKind::Klondike,
            42,
        );
        let viewport = Size::new(1024.0, 768.0);
        app.layout(viewport);
        // HUD strip lives at y=[26, 74] in Y-up (immediately
        // above the menu bar at y=[0, 26]). Pick a click point
        // safely inside that band and near the centre of the
        // viewport — that's where the buttons centre-align.
        let click = Point::new(viewport.width * 0.5, 50.0);
        let path = hit_test_subtree(app.root(), click)
            .expect("HUD-strip click must hit something");
        let chain = type_chain_for_path(app.root(), &path);
        assert!(
            chain.contains(&"HudWidget"),
            "HUD-strip click must reach HudWidget, but landed on {chain:?}"
        );
        assert!(
            chain.contains(&"Button"),
            "HUD-strip click must reach a Button child, but landed on {chain:?}"
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
