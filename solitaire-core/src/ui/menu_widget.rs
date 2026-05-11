//! Game/Options/Help menu — desktop convention puts it across the top
//! of the viewport as a horizontal bar (`MenuBarHost`). On landscape-
//! mobile-shaped viewports we hide the top bar entirely and re-host the
//! same menus as a VERTICAL strip pinned to the top of the left sidebar
//! (`SidebarMenuHost`), with popups opening to the right.
//!
//! Both hosts wrap an `agg_gui::widgets::menu::MenuBar` and route
//! action strings through a shared [`handle_action`] table back into
//! [`AppModel`].

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::menu::{MenuBar, MenuEntry, MenuItem, MenuOrientation, TopMenu, MENU_BAR_H};

use super::app_model::{AppModel, HelpKind, Screen, SharedModel};
use super::layout::{self, ChromeMode, SIDEBAR_MENU_H, SIDEBAR_W};

/// Horizontal menu bar across the top of the viewport. Hidden in
/// sidebar mode (its actions are mirrored by `SidebarMenuHost`).
pub struct MenuBarHost {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl MenuBarHost {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let bar = build_menu_bar(model.clone(), font, MenuOrientation::Horizontal);
        Self {
            bounds: Rect::default(),
            children: vec![Box::new(bar)],
            model,
        }
    }
}

/// Vertical menu strip pinned to the TOP of the left sidebar. Only
/// visible in `ChromeMode::Sidebar`. Each top menu (Game / Options /
/// Help) is a row that opens its popup to the right.
pub struct SidebarMenuHost {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl SidebarMenuHost {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let bar = build_menu_bar(model.clone(), font, MenuOrientation::Vertical);
        Self {
            bounds: Rect::default(),
            children: vec![Box::new(bar)],
            model,
        }
    }
}

fn build_menu_bar(model: SharedModel, font: Arc<Font>, orientation: MenuOrientation) -> MenuBar {
    let menus = build_menus(&model.borrow());
    let model_for_action = model.clone();
    MenuBar::new(font, menus, move |action| {
        let mut m = model_for_action.borrow_mut();
        handle_action(&mut m, action);
        agg_gui::animation::request_draw();
    })
    .with_orientation(orientation)
}

/// Build the menus from current model state. Called once at construction;
/// `MenuBar` then owns the items and mutates radio/check selection on
/// click. We don't rebuild on every frame — that would lose the
/// transient hover/open state the bar tracks internally.
fn build_menus(model: &AppModel) -> Vec<TopMenu> {
    let draw = model.klondike_draw_count;
    vec![
        TopMenu::new(
            "Game",
            vec![
                MenuItem::action("New Deal", "new-deal")
                    .shortcut("F2")
                    .into(),
                MenuItem::action("Restart this Deal", "restart").into(),
                MenuEntry::Separator,
                MenuItem::action("Back to Title", "title").into(),
            ],
        ),
        TopMenu::new(
            "Options",
            vec![
                MenuItem::action("Draw 1", "draw-1")
                    .radio(draw == 1)
                    .keep_open()
                    .into(),
                MenuItem::action("Draw 3", "draw-3")
                    .radio(draw == 3)
                    .keep_open()
                    .into(),
                MenuEntry::Separator,
                MenuItem::action("Toggle Fullscreen", "toggle-fullscreen").into(),
            ],
        ),
        TopMenu::new(
            "Help",
            vec![
                // Both items dispatch by `model.kind` so the player only
                // ever sees content for the variant they're playing.
                MenuItem::action("Rules", "help-rules").into(),
                MenuItem::action("About\u{2026}", "help-about").into(),
            ],
        ),
    ]
}

fn handle_action(model: &mut AppModel, action: &str) {
    match action {
        "new-deal" => {
            if let Some(kind) = model.kind {
                model.start_game(kind);
            }
        }
        "restart" => model.restart_current_deal(),
        "title" => model.back_to_title(),
        "draw-1" => model.set_klondike_draw_count(1),
        "draw-3" => model.set_klondike_draw_count(3),
        "help-rules" => model.help = model.kind.map(HelpKind::Rules),
        "help-about" => model.help = model.kind.map(HelpKind::About),
        "toggle-fullscreen" => crate::platform::request_toggle_fullscreen(),
        _ => {}
    }
}

impl Widget for MenuBarHost {
    fn type_name(&self) -> &'static str {
        "MenuBarHost"
    }
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // Y-up: top of the window is at y = bounds.y + bounds.height. The
        // bar sits in the top BAR_H pixels.
        let bar_y = bounds.y + bounds.height - MENU_BAR_H;
        let bar_rect = Rect::new(bounds.x, bar_y, bounds.width, MENU_BAR_H);
        if let Some(bar) = self.children.first_mut() {
            bar.set_bounds(bar_rect);
        }
    }
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }
    fn layout(&mut self, available: Size) -> Size {
        if let Some(bar) = self.children.first_mut() {
            bar.layout(Size::new(available.width, MENU_BAR_H));
        }
        available
    }
    fn is_visible(&self) -> bool {
        let s = self.model.borrow().screen;
        if !matches!(s, Screen::Game | Screen::Won) {
            return false;
        }
        // Hide the horizontal menu bar in sidebar mode — the same
        // actions are exposed as vertical buttons in HudWidget so the
        // top 26 px of viewport can go to the playfield.
        let chrome = layout::compute(Size::new(self.bounds.width, self.bounds.height));
        chrome.mode != ChromeMode::Sidebar
    }
    /// Claim only the top BAR_H pixels for ordinary input; without this
    /// the OverlayStack's top→bottom hit-test stops at us (full window
    /// bounds) and never reaches HudWidget / GameWidget below — same
    /// gotcha HudWidget calls out in its own `hit_test` override.
    /// Open-popup events go through `has_active_modal` on the inner
    /// `MenuBar` so we don't need to forward those here.
    fn hit_test(&self, local_pos: Point) -> bool {
        if !self.is_visible() {
            return false;
        }
        let top = self.bounds.height;
        let bottom = self.bounds.height - MENU_BAR_H;
        local_pos.y >= bottom && local_pos.y <= top
    }
    fn paint(&mut self, _ctx: &mut dyn DrawCtx) {
        // Bar paints itself via the framework's tree walk; we have no
        // body of our own.
    }
    fn on_event(&mut self, _event: &Event) -> EventResult {
        // Same: events route to the bar child via tree dispatch.
        EventResult::Ignored
    }
    fn needs_draw(&self) -> bool {
        self.children.iter().any(|c| c.needs_draw())
    }
}

impl Widget for SidebarMenuHost {
    fn type_name(&self) -> &'static str {
        "SidebarMenuHost"
    }
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // Pin the vertical menu strip to the TOP of the left sidebar
        // column. Y-up: highest local Y is at the top.
        let strip_x = bounds.x;
        let strip_w = SIDEBAR_W;
        let strip_h = SIDEBAR_MENU_H;
        let strip_y = bounds.y + bounds.height - strip_h;
        let bar_rect = Rect::new(strip_x, strip_y, strip_w, strip_h);
        if let Some(bar) = self.children.first_mut() {
            bar.set_bounds(bar_rect);
        }
    }
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }
    fn layout(&mut self, available: Size) -> Size {
        if let Some(bar) = self.children.first_mut() {
            bar.layout(Size::new(SIDEBAR_W, SIDEBAR_MENU_H));
        }
        available
    }
    fn is_visible(&self) -> bool {
        let s = self.model.borrow().screen;
        if !matches!(s, Screen::Game | Screen::Won) {
            return false;
        }
        let chrome = layout::compute(Size::new(self.bounds.width, self.bounds.height));
        chrome.mode == ChromeMode::Sidebar
    }
    fn hit_test(&self, local_pos: Point) -> bool {
        if !self.is_visible() {
            return false;
        }
        // Claim only the top-left strip occupied by the vertical menu
        // so playfield clicks pass through to GameWidget.
        let top = self.bounds.height;
        let bottom = self.bounds.height - SIDEBAR_MENU_H;
        local_pos.x >= 0.0
            && local_pos.x <= SIDEBAR_W
            && local_pos.y >= bottom
            && local_pos.y <= top
    }
    fn paint(&mut self, _ctx: &mut dyn DrawCtx) {}
    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }
    fn needs_draw(&self) -> bool {
        self.children.iter().any(|c| c.needs_draw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GameKind;

    #[test]
    fn draw_count_action_updates_model() {
        let mut m = AppModel::new();
        assert_eq!(m.klondike_draw_count, 1);
        handle_action(&mut m, "draw-3");
        assert_eq!(m.klondike_draw_count, 3);
        handle_action(&mut m, "draw-1");
        assert_eq!(m.klondike_draw_count, 1);
    }

    #[test]
    fn draw_count_change_during_klondike_restarts_with_same_seed() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Klondike, 42);
        let pre_seed = m.session.as_ref().unwrap().seed();
        m.set_klondike_draw_count(3);
        let post_seed = m.session.as_ref().unwrap().seed();
        assert_eq!(pre_seed, post_seed);
        // Waste pile fan should now be active because draw_count = 3.
        let session = m.session.as_ref().unwrap();
        let waste = session.piles().get(crate::games::klondike::KLONDIKE_WASTE);
        assert_eq!(waste.fan_top_n, 3);
    }

    #[test]
    fn changing_draw_count_during_freecell_does_not_restart() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::FreeCell, 99);
        let pre_slug = m.session.as_ref().unwrap().game_slug();
        m.set_klondike_draw_count(3);
        // FreeCell session is preserved (still freecell, same seed).
        let post = m.session.as_ref().unwrap();
        assert_eq!(post.game_slug(), pre_slug);
        assert_eq!(post.seed(), 99);
        assert_eq!(m.klondike_draw_count, 3);
    }
}
