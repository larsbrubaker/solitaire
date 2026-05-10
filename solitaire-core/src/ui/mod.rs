//! Shared Solitaire widget tree and UI builders.
//!
//! Native and WASM shells must build the game through this module instead of
//! constructing widgets directly. Platform crates own only OS/browser wiring;
//! every game screen, menu, layout, and widget tree lives here.

use std::sync::Arc;

use agg_gui::text::Font;
use agg_gui::App;

pub mod app_model;
pub mod app_root;
pub mod dyn_session;
pub mod game_widget;
pub mod hud_widget;
pub mod icons;
pub mod menu_widget;
pub mod overlay_stack;
pub mod title_widget;

use app_model::shared_model;
use app_root::AppRootWidget;
use game_widget::GameWidget;
use hud_widget::HudWidget;
use menu_widget::MenuBarHost;
use overlay_stack::OverlayStack;
use title_widget::TitleWidget;

use crate::render::CardSpriteAtlas;

/// CascadiaCode bundled into the binary.
const FONT_BYTES: &[u8] = include_bytes!("../../assets/CascadiaCode.ttf");

fn load_default_font() -> Arc<Font> {
    Arc::new(Font::from_slice(FONT_BYTES).expect("solitaire default font"))
}

/// Build the shared Solitaire application. Returns the [`App`] hosting the
/// widget tree (title screen + game widget + HUD, switched via the
/// shared `AppModel`).
pub fn build_solitaire_app() -> App {
    let model = shared_model();
    let font = load_default_font();
    // Pre-rasterise the 53 unique card sprites at a default 1× scale.
    // GameWidget rebuilds the atlas at the actual render scale once the
    // first paint runs and the playfield bounds + device DPR are known.
    let atlas = CardSpriteAtlas::build(&font, 1.0);

    let title = TitleWidget::new(model.clone(), font.clone());
    let game = GameWidget::new(model.clone(), font.clone(), atlas);
    let hud = HudWidget::new(model.clone(), font.clone());
    let menu = MenuBarHost::new(model.clone(), font.clone());
    let root = AppRootWidget::new(model.clone());

    // Painted bottom→top, hit-tested top→bottom.
    let stack = OverlayStack::new()
        .add(Box::new(root))
        .add(Box::new(game))
        .add(Box::new(hud))
        .add(Box::new(menu))
        .add(Box::new(title));

    App::new(Box::new(stack))
}
