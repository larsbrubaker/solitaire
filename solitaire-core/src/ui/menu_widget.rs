//! Game/Options/Help menu — desktop convention puts it across the top
//! of the viewport as a horizontal bar (`MenuBarHost`). On landscape-
//! mobile-shaped viewports we hide the top bar entirely and re-host the
//! same menus as a VERTICAL strip pinned to the top of the left sidebar
//! (`SidebarMenuHost`), with popups opening to the right.
//!
//! Both hosts wrap an `agg_gui::widgets::menu::MenuBar` and route
//! action strings through a shared [`handle_action`] table back into
//! [`AppModel`]. The Options menu is per-variant — Draw 1/3 only shows
//! for Klondike, Spider's suit-count + one-suit-choice picker only
//! shows for Spider, etc. Hosts watch `model.kind` and rebuild the
//! inner `MenuBar` whenever the active variant changes.

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::menu::{MenuBar, MenuEntry, MenuItem, MenuOrientation, TopMenu, MENU_BAR_H};

use crate::cards::Suit;
use crate::games::GameKind;

use super::app_model::{AppModel, HelpKind, Screen, SharedModel};
use super::layout::{self, ChromeMode, SIDEBAR_MENU_H, SIDEBAR_W};

/// Snapshot of every `AppModel` field that affects the menu's text or
/// radio state. The hosts compare a freshly-computed snapshot against
/// the last-built one each paint and rebuild the inner `MenuBar` if
/// anything changed — without this, switching Spider from 4-suit to
/// 1-suit via the Options menu left the "● 2 Suits" / "● 4 Suits"
/// radio dots stale (the radios were locked in at construction).
#[derive(Clone, Copy, PartialEq, Eq)]
struct MenuSnapshot {
    kind: Option<GameKind>,
    klondike_draw_count: u8,
    spider_suit_count: u8,
    spider_one_suit: Suit,
}

impl MenuSnapshot {
    fn from(model: &AppModel) -> Self {
        Self {
            kind: model.kind,
            klondike_draw_count: model.klondike_draw_count,
            spider_suit_count: model.spider_suit_count,
            spider_one_suit: model.spider_one_suit,
        }
    }
}

/// Horizontal menu bar across the top of the viewport. Hidden in
/// sidebar mode (its actions are mirrored by `SidebarMenuHost`).
pub struct MenuBarHost {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    current_snapshot: MenuSnapshot,
}

impl MenuBarHost {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let snapshot = MenuSnapshot::from(&model.borrow());
        let bar = build_menu_bar(model.clone(), font.clone(), MenuOrientation::Horizontal);
        Self {
            bounds: Rect::default(),
            children: vec![Box::new(bar)],
            model,
            font,
            current_snapshot: snapshot,
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
    font: Arc<Font>,
    current_snapshot: MenuSnapshot,
}

impl SidebarMenuHost {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let snapshot = MenuSnapshot::from(&model.borrow());
        let bar = build_menu_bar(model.clone(), font.clone(), MenuOrientation::Vertical);
        Self {
            bounds: Rect::default(),
            children: vec![Box::new(bar)],
            model,
            font,
            current_snapshot: snapshot,
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

/// Build the menu structure for the variant currently in `model`. The
/// Game and Help menus are universal; the Options menu is per-variant.
fn build_menus(model: &AppModel) -> Vec<TopMenu> {
    let kind = model.kind;
    let mut out = vec![game_menu(), options_menu(model, kind), help_menu()];
    // Title screen — no variant selected. Drop the Options menu since
    // every entry is per-variant. (Toggle Fullscreen will reappear once
    // the player picks a game.)
    if kind.is_none() {
        out.remove(1);
    }
    out
}

fn game_menu() -> TopMenu {
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
    )
}

fn help_menu() -> TopMenu {
    TopMenu::new(
        "Help",
        vec![
            // Both items dispatch by `model.kind` so the player only
            // ever sees content for the variant they're playing.
            MenuItem::action("Rules", "help-rules").into(),
            MenuItem::action("About\u{2026}", "help-about").into(),
        ],
    )
}

fn options_menu(model: &AppModel, kind: Option<GameKind>) -> TopMenu {
    let mut items: Vec<MenuEntry> = Vec::new();
    match kind {
        Some(GameKind::Klondike) => {
            let draw = model.klondike_draw_count;
            items.push(
                MenuItem::action("Draw 1", "draw-1")
                    .radio(draw == 1)
                    .keep_open()
                    .into(),
            );
            items.push(
                MenuItem::action("Draw 3", "draw-3")
                    .radio(draw == 3)
                    .keep_open()
                    .into(),
            );
            items.push(MenuEntry::Separator);
        }
        Some(GameKind::Spider) => {
            let count = model.spider_suit_count;
            // 1-suit Spider hosts a sub-menu where the player picks the
            // active suit; the parent row's radio reflects whether
            // 1-suit mode is the current count, the children's radios
            // reflect which suit is selected within 1-suit mode.
            items.push(
                MenuItem::submenu(
                    "1 Suit",
                    vec![
                        spider_one_suit_item(model, Suit::Spades, "Spades", "spider-suit-spades"),
                        spider_one_suit_item(model, Suit::Hearts, "Hearts", "spider-suit-hearts"),
                        spider_one_suit_item(
                            model,
                            Suit::Diamonds,
                            "Diamonds",
                            "spider-suit-diamonds",
                        ),
                        spider_one_suit_item(model, Suit::Clubs, "Clubs", "spider-suit-clubs"),
                    ],
                )
                .radio(count == 1)
                .into(),
            );
            items.push(
                MenuItem::action("2 Suits", "spider-2-suit")
                    .radio(count == 2)
                    .keep_open()
                    .into(),
            );
            items.push(
                MenuItem::action("4 Suits", "spider-4-suit")
                    .radio(count == 4)
                    .keep_open()
                    .into(),
            );
            items.push(MenuEntry::Separator);
        }
        _ => {}
    }
    items.push(MenuItem::action("Toggle Fullscreen", "toggle-fullscreen").into());
    TopMenu::new("Options", items)
}

fn spider_one_suit_item(model: &AppModel, suit: Suit, label: &str, action: &str) -> MenuEntry {
    // Radio fires only when 1-suit Spider is already active AND this is
    // the chosen suit. Picking a suit while in 2/4-suit mode SWITCHES
    // to 1-suit mode with the chosen suit (handled in handle_action).
    let selected = model.spider_suit_count == 1 && model.spider_one_suit == suit;
    MenuItem::action(label.to_string(), action.to_string())
        .radio(selected)
        .keep_open()
        .into()
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
        "spider-2-suit" => model.set_spider_suit_count(2),
        "spider-4-suit" => model.set_spider_suit_count(4),
        "spider-suit-spades" => spider_set_one_suit(model, Suit::Spades),
        "spider-suit-hearts" => spider_set_one_suit(model, Suit::Hearts),
        "spider-suit-diamonds" => spider_set_one_suit(model, Suit::Diamonds),
        "spider-suit-clubs" => spider_set_one_suit(model, Suit::Clubs),
        "help-rules" => model.help = model.kind.map(HelpKind::Rules),
        "help-about" => model.help = model.kind.map(HelpKind::About),
        "toggle-fullscreen" => crate::platform::request_toggle_fullscreen(),
        _ => {}
    }
}

/// Pick `suit` AND switch to 1-suit Spider (re-deal). Picking a 1-suit
/// item while 2/4-suit is active should switch modes — that's what the
/// player asked for. The setters handle the no-op-when-unchanged case.
fn spider_set_one_suit(model: &mut AppModel, suit: Suit) {
    model.set_spider_one_suit(suit);
    model.set_spider_suit_count(1);
}

impl MenuBarHost {
    /// Rebuild the inner `MenuBar` if any menu-visible state has
    /// changed since last sync — variant, draw count, Spider suit
    /// count + active suit. Called from `paint()` so per-frame menu
    /// rebuilds pick up state changes. The rebuild discards transient
    /// hover/open state on the bar — acceptable because the changes
    /// that trigger a rebuild (radio click in Options, "New Deal",
    /// etc.) close the popup on their own first.
    fn sync_state(&mut self) {
        let snapshot = MenuSnapshot::from(&self.model.borrow());
        if self.current_snapshot == snapshot {
            return;
        }
        self.current_snapshot = snapshot;
        let bar = build_menu_bar(
            self.model.clone(),
            self.font.clone(),
            MenuOrientation::Horizontal,
        );
        self.children = vec![Box::new(bar)];
        let bar_y = self.bounds.y + self.bounds.height - MENU_BAR_H;
        let bar_rect = Rect::new(self.bounds.x, bar_y, self.bounds.width, MENU_BAR_H);
        if let Some(bar) = self.children.first_mut() {
            bar.layout(Size::new(self.bounds.width, MENU_BAR_H));
            bar.set_bounds(bar_rect);
        }
        agg_gui::animation::request_draw();
    }
}

impl SidebarMenuHost {
    fn sync_state(&mut self) {
        let snapshot = MenuSnapshot::from(&self.model.borrow());
        if self.current_snapshot == snapshot {
            return;
        }
        self.current_snapshot = snapshot;
        let bar = build_menu_bar(
            self.model.clone(),
            self.font.clone(),
            MenuOrientation::Vertical,
        );
        self.children = vec![Box::new(bar)];
        let strip_y = self.bounds.y + self.bounds.height - SIDEBAR_MENU_H;
        let bar_rect = Rect::new(self.bounds.x, strip_y, SIDEBAR_W, SIDEBAR_MENU_H);
        if let Some(bar) = self.children.first_mut() {
            bar.layout(Size::new(SIDEBAR_W, SIDEBAR_MENU_H));
            bar.set_bounds(bar_rect);
        }
        agg_gui::animation::request_draw();
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
        // actions are exposed by `SidebarMenuHost` in the left column.
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
        self.sync_state();
    }
    fn on_event(&mut self, _event: &Event) -> EventResult {
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
        let top = self.bounds.height;
        let bottom = self.bounds.height - SIDEBAR_MENU_H;
        local_pos.x >= 0.0
            && local_pos.x <= SIDEBAR_W
            && local_pos.y >= bottom
            && local_pos.y <= top
    }
    fn paint(&mut self, _ctx: &mut dyn DrawCtx) {
        self.sync_state();
    }
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
        let post = m.session.as_ref().unwrap();
        assert_eq!(post.game_slug(), pre_slug);
        assert_eq!(post.seed(), 99);
        assert_eq!(m.klondike_draw_count, 3);
    }

    #[test]
    fn spider_suit_count_change_during_spider_re_deals() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Spider, 7);
        assert_eq!(m.spider_suit_count, 4);
        m.set_spider_suit_count(1);
        assert_eq!(m.spider_suit_count, 1);
        // Same seed, so the same shuffle slots are filled — but the
        // deck only has one suit's cards in it now.
        let session = m.session.as_ref().unwrap();
        assert_eq!(session.seed(), 7);
        // Verify the deck composition: every face-up card in the
        // cascade tops should be the same suit as the configured
        // `spider_one_suit` (Spades by default).
        let piles = session.piles();
        for cid in 9..=18u8 {
            let top = piles.get(cid).top().unwrap();
            assert_eq!(top.suit, m.spider_one_suit);
        }
    }

    #[test]
    fn picking_spider_one_suit_switches_to_one_suit_mode() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Spider, 11);
        assert_eq!(m.spider_suit_count, 4);
        handle_action(&mut m, "spider-suit-hearts");
        assert_eq!(m.spider_suit_count, 1);
        assert_eq!(m.spider_one_suit, Suit::Hearts);
    }
}
