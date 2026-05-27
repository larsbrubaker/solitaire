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

use super::app_model::{AppModel, HelpKind, SharedModel};
use super::layout;

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
    spider_winnable_only: bool,
    freecell_winnable_only: bool,
    klondike_winnable_only: bool,
    /// Active session's seed — drives the deal-info row in the
    /// Game menu so it rebuilds when New Deal fires.
    session_seed: Option<u64>,
    /// Performance-window visibility: drives the radio-dot indicator
    /// on the "Performance Window" menu item.
    show_performance_window: bool,
    /// Whether a game is currently being played — drives whether
    /// Undo / Hint / Shuffle appear in the Game menu.
    in_game: bool,
}

impl MenuSnapshot {
    fn from(model: &AppModel) -> Self {
        Self {
            kind: model.kind,
            klondike_draw_count: model.klondike_draw_count,
            spider_suit_count: model.spider_suit_count,
            spider_one_suit: model.spider_one_suit,
            spider_winnable_only: model.spider_winnable_only,
            freecell_winnable_only: model.freecell_winnable_only,
            klondike_winnable_only: model.klondike_winnable_only,
            show_performance_window: model.show_performance_window.get(),
            in_game: model.session.is_some(),
            session_seed: model.session.as_ref().map(|s| s.seed()),
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
        // Top-of-viewport chrome — `Horizontal` opens popups
        // DOWNWARD (toward smaller y in Y-up) so the menu drops
        // into the playfield area below.
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

// `SidebarMenuHost` removed — the bottom-bar layout is the only
// chrome mode now (see `layout::compute`).

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

/// Build the menu structure for the variant currently in `model`.
///
/// Single top-level "Menu" entry whose popup is a 3-row list of
/// **Game / Options / Help** submenus — i.e. one button on the
/// menu bar, two clicks to reach any leaf action. The previous
/// three-button bar (Game / Options / Help) was driving the
/// bar's natural width past the available HUD strip in narrow
/// viewports; nesting it all under one entry keeps the bar
/// compact and matches the "hamburger of menus" pattern.
fn build_menus(model: &AppModel) -> Vec<TopMenu> {
    let mut items: Vec<MenuEntry> = Vec::new();
    if model.session.is_some() {
        items.push(MenuItem::submenu("Game", game_menu_items(model)).into());
    }
    items.push(MenuItem::submenu("Options", options_menu_items(model, model.kind)).into());
    items.push(MenuItem::submenu("Help", help_menu_items(model)).into());
    vec![TopMenu::new("Menu", items)]
}

fn game_menu_items(model: &AppModel) -> Vec<MenuEntry> {
    let mut items: Vec<MenuEntry> = Vec::new();
    if model.session.is_some() {
        items.push(
            MenuItem::action("Undo", "undo")
                .shortcut("U / Ctrl+Z")
                .into(),
        );
        match model.kind {
            Some(GameKind::Spider) | Some(GameKind::Klondike) => {
                items.push(MenuItem::action("Hint", "hint").shortcut("H").into());
            }
            Some(GameKind::MomsSolitaire) => {
                items.push(MenuItem::action("Shuffle", "shuffle").into());
            }
            _ => {}
        }
        items.push(MenuEntry::Separator);
    }
    items.push(
        MenuItem::action("New Deal", "new-deal")
            .shortcut("F2")
            .into(),
    );
    // Show the active deal's identifier as an info row directly
    // above "Play Deal Number…". Disabled so it can't be clicked;
    // it's there for players who want to write the number down or
    // share it. Only shown when a game is in progress.
    if let Some(label) = current_deal_label(model) {
        items.push(MenuItem::action(label, "deal-info").disabled().into());
    }
    items.push(MenuItem::action("Play Deal Number\u{2026}", "play-deal-number").into());
    items.push(MenuItem::action("Restart this Deal", "restart").into());
    items.push(MenuEntry::Separator);
    items.push(MenuItem::action("Back to Main Menu", "title").into());
    items
}

/// Format the active session's deal identifier for the disabled
/// info row above "Play Deal Number…". `None` if no game is in
/// progress. FreeCell in winnable-only mode shows the Microsoft
/// game number in decimal so players can share / recall a
/// familiar `Game #11234`; other variants show the raw `u64` seed
/// in hex.
fn current_deal_label(model: &AppModel) -> Option<String> {
    let seed = model.session.as_ref()?.seed();
    let kind = model.kind?;
    let label = match kind {
        GameKind::FreeCell if model.freecell_winnable_only => format!("Game #{}", seed),
        _ => format!("Deal #{:016x}", seed),
    };
    Some(label)
}

fn help_menu_items(model: &AppModel) -> Vec<MenuEntry> {
    let mut items: Vec<MenuEntry> = Vec::new();
    // Per-game About sits ON TOP when a game is active. The suite-
    // level About below it talks about OneAndDone.games and the
    // overall app.
    if let Some(kind) = model.kind {
        let label = format!("About {}\u{2026}", kind.display_name());
        items.push(MenuItem::action(label, "help-about").into());
    }
    items.push(MenuItem::action("About\u{2026}", "help-about-suite").into());
    items
}

fn options_menu_items(model: &AppModel, kind: Option<GameKind>) -> Vec<MenuEntry> {
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
            items.push(
                MenuItem::action("Winnable deals only", "klondike-winnable-only")
                    .radio(model.klondike_winnable_only)
                    .keep_open()
                    .into(),
            );
            items.push(MenuEntry::Separator);
        }
        Some(GameKind::FreeCell) => {
            items.push(
                MenuItem::action("Winnable deals only", "freecell-winnable-only")
                    .radio(model.freecell_winnable_only)
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
            items.push(
                MenuItem::action("Winnable deals only", "spider-winnable-only")
                    .radio(model.spider_winnable_only)
                    .keep_open()
                    .into(),
            );
            items.push(MenuEntry::Separator);
        }
        _ => {}
    }
    items.push(MenuItem::action("Toggle Fullscreen", "toggle-fullscreen").into());
    items.push(MenuEntry::Separator);
    items.push(debug_menu(model).into());
    items
}

/// Developer-only submenu nested under Options.  Currently hosts a
/// single toggle for the Performance window (Mean CPU usage +
/// sparkline); future debug overlays should slot in as additional
/// entries here so they all live behind one parent label.
fn debug_menu(model: &AppModel) -> MenuItem {
    let busy = crate::games::seed_generator::seed_generation_running();
    let label = if busy {
        "Generate Seed Games\u{2026} (running)"
    } else {
        "Generate Seed Games\u{2026}"
    };
    MenuItem::submenu(
        "Debug",
        vec![
            MenuItem::action("Performance Window", "toggle-performance-window")
                .radio(model.show_performance_window.get())
                .keep_open()
                .into(),
            MenuEntry::Separator,
            MenuItem::action(label, "generate-seeds").into(),
        ],
    )
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
        "undo" => {
            if let Some(s) = model.session.as_mut() {
                s.try_undo();
            }
            model.clear_spider_hint();
        }
        "hint" => model.show_hint(),
        "shuffle" => {
            model.try_moms_shuffle();
        }
        "new-deal" => {
            if let Some(kind) = model.kind {
                model.request_new_deal(kind);
            }
        }
        "restart" => model.restart_current_deal(),
        "play-deal-number" => model.open_play_deal_dialog(),
        "title" => model.request_main_menu(),
        "draw-1" => model.set_klondike_draw_count(1),
        "draw-3" => model.set_klondike_draw_count(3),
        "spider-2-suit" => model.set_spider_suit_count(2),
        "spider-4-suit" => model.set_spider_suit_count(4),
        "spider-winnable-only" => model.set_spider_winnable_only(!model.spider_winnable_only),
        "freecell-winnable-only" => model.set_freecell_winnable_only(!model.freecell_winnable_only),
        "klondike-winnable-only" => model.set_klondike_winnable_only(!model.klondike_winnable_only),
        "spider-suit-spades" => spider_set_one_suit(model, Suit::Spades),
        "spider-suit-hearts" => spider_set_one_suit(model, Suit::Hearts),
        "spider-suit-diamonds" => spider_set_one_suit(model, Suit::Diamonds),
        "spider-suit-clubs" => spider_set_one_suit(model, Suit::Clubs),
        "help-rules" => model.help = model.kind.map(HelpKind::Rules),
        "help-about" => model.help = model.kind.map(HelpKind::About),
        "help-about-suite" => model.help = Some(HelpKind::AboutSuite),
        "toggle-fullscreen" => crate::platform::request_toggle_fullscreen(),
        "toggle-performance-window" => {
            let now_open = model.show_performance_window.get();
            model.set_performance_window_open(!now_open)
        }
        "generate-seeds" => {
            crate::games::seed_generator::start_seed_generation();
            model.set_seed_gen_window_open(true);
        }
        "stop-seed-generation" => {
            crate::games::seed_generator::stop_seed_generation();
        }
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
        // Menu cascade button shares the top strip with the HUD
        // action buttons. Position the inner bar inside the menu
        // slice with a vertical centring offset so its 26 px
        // height sits in the middle of the 48 px strip.
        let chrome = super::layout::compute(Size::new(self.bounds.width, self.bounds.height));
        let bar_rect = inner_bar_rect(chrome.menu_rect);
        if let Some(bar) = self.children.first_mut() {
            bar.layout(Size::new(bar_rect.width, MENU_BAR_H));
            bar.set_bounds(bar_rect);
        }
        agg_gui::animation::request_draw();
    }
}

/// Position the inner `MenuBar` widget INSIDE the chrome's menu
/// slice — vertically centred so the 26 px bar sits in the middle
/// of the 48 px combined strip and lines up visually with the
/// action buttons next to it.
fn inner_bar_rect(menu_slot: Rect) -> Rect {
    let y = menu_slot.y + (menu_slot.height - MENU_BAR_H) * 0.5;
    Rect::new(menu_slot.x, y, menu_slot.width, MENU_BAR_H)
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
        // Position the inner `MenuBar` inside the menu slice of
        // the shared top strip — `inner_bar_rect` vertically
        // centres the 26 px bar inside the 48 px strip so it lines
        // up with the HUD action buttons next to it.
        let chrome = layout::compute(Size::new(bounds.width, bounds.height));
        if let Some(bar) = self.children.first_mut() {
            bar.set_bounds(inner_bar_rect(chrome.menu_rect));
        }
    }
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }
    fn layout(&mut self, available: Size) -> Size {
        let chrome = layout::compute(available);
        let bar_rect = inner_bar_rect(chrome.menu_rect);
        if let Some(bar) = self.children.first_mut() {
            bar.layout(Size::new(bar_rect.width, MENU_BAR_H));
        }
        available
    }
    fn is_visible(&self) -> bool {
        // Visible on every screen — the title screen wants the
        // same Options/Help menus the gameplay screens get (so
        // e.g. the Debug submenu and Toggle Fullscreen are always
        // reachable). The chrome strip lives at the top of the
        // viewport regardless of which screen is active.
        true
    }
    /// Claim only the menu-slice rect of the shared top strip;
    /// without this the OverlayStack's top→bottom hit-test stops
    /// at us (full window bounds) and never reaches HudWidget /
    /// GameWidget below — same gotcha HudWidget calls out in its
    /// own `hit_test` override. Open-popup events go through
    /// `has_active_modal` on the inner `MenuBar` so we don't need
    /// to forward those here.
    fn hit_test(&self, local_pos: Point) -> bool {
        if !self.is_visible() {
            return false;
        }
        let m = layout::compute(Size::new(self.bounds.width, self.bounds.height)).menu_rect;
        local_pos.x >= m.x
            && local_pos.x <= m.x + m.width
            && local_pos.y >= m.y
            && local_pos.y <= m.y + m.height
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
    use crate::ui::app_model::ConfirmAction;

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
    fn draw_count_change_on_fresh_klondike_redeals_immediately() {
        // Fresh deal has no moves yet, so the change applies without
        // a confirm prompt and the visible board re-deals so the
        // player sees the new rules at once.
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Klondike, 42);
        m.set_klondike_draw_count(3);
        assert_eq!(m.klondike_draw_count, 3);
        assert_eq!(m.confirm, None, "no confirm needed for a fresh deal");
        let waste = m
            .session
            .as_ref()
            .unwrap()
            .piles()
            .get(crate::games::klondike::KLONDIKE_WASTE);
        assert_eq!(waste.fan_top_n, 3, "active deal switched to draw-3 rules");
    }

    #[test]
    fn draw_count_change_with_moves_prompts_first() {
        // After a move there's progress to lose, so the setter queues
        // a confirm action instead of re-dealing silently.
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Klondike, 42);
        // Click the stock to make the deal "in progress" (counts as a
        // user move on the undo stack).
        let session = m.session.as_mut().unwrap();
        let moves = session.on_pile_click(crate::games::klondike::KLONDIKE_STOCK);
        assert!(!moves.is_empty(), "fresh Klondike stock should deal");
        assert!(session.try_apply_batch(moves));
        let original_seed = m.session.as_ref().unwrap().seed();

        m.set_klondike_draw_count(3);
        assert_eq!(
            m.confirm,
            Some(ConfirmAction::ApplyKlondikeDrawCount(3)),
            "setter must queue the confirm instead of applying"
        );
        assert_eq!(
            m.klondike_draw_count, 1,
            "setting must NOT change until the user confirms"
        );
        assert_eq!(m.session.as_ref().unwrap().seed(), original_seed);

        // Confirm — now setting changes and the deal restarts.
        m.confirm_pending_action();
        assert_eq!(m.klondike_draw_count, 3);
        assert_eq!(m.confirm, None);
        assert_eq!(
            m.session.as_ref().unwrap().seed(),
            original_seed,
            "restart_current_deal keeps the same seed"
        );
    }

    #[test]
    fn draw_count_change_cancelled_reverts_setting() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Klondike, 42);
        let session = m.session.as_mut().unwrap();
        let moves = session.on_pile_click(crate::games::klondike::KLONDIKE_STOCK);
        assert!(session.try_apply_batch(moves));

        m.set_klondike_draw_count(3);
        m.cancel_pending_action();
        assert_eq!(m.confirm, None);
        assert_eq!(
            m.klondike_draw_count, 1,
            "cancel keeps the original setting"
        );
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
    fn spider_suit_count_change_on_fresh_deal_redeals_immediately() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Spider, 7);
        let pre_piles: Vec<Vec<_>> = m
            .session
            .as_ref()
            .unwrap()
            .piles()
            .iter()
            .map(|p| p.cards.clone())
            .collect();
        m.set_spider_suit_count(2);
        assert_eq!(m.spider_suit_count, 2);
        assert_eq!(m.confirm, None);
        let post_piles: Vec<Vec<_>> = m
            .session
            .as_ref()
            .unwrap()
            .piles()
            .iter()
            .map(|p| p.cards.clone())
            .collect();
        assert_ne!(
            pre_piles, post_piles,
            "fresh deal re-shuffled under the new suit count"
        );
    }

    #[test]
    fn spider_suit_count_change_with_moves_prompts_first() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Spider, 7);
        // Stock click counts as a user move on Spider.
        let session = m.session.as_mut().unwrap();
        let moves = session.on_pile_click(8);
        assert!(session.try_apply_batch(moves));
        let pre_piles: Vec<Vec<_>> = m
            .session
            .as_ref()
            .unwrap()
            .piles()
            .iter()
            .map(|p| p.cards.clone())
            .collect();
        m.set_spider_suit_count(2);
        assert_eq!(m.confirm, Some(ConfirmAction::ApplySpiderSuitCount(2)));
        assert_eq!(m.spider_suit_count, 1, "setting waits for confirm");
        let post_piles: Vec<Vec<_>> = m
            .session
            .as_ref()
            .unwrap()
            .piles()
            .iter()
            .map(|p| p.cards.clone())
            .collect();
        assert_eq!(pre_piles, post_piles, "active piles unchanged pre-confirm");
    }

    #[test]
    fn picking_spider_one_suit_on_fresh_deal_redeals() {
        let mut m = AppModel::new();
        m.start_game_with_seed(GameKind::Spider, 11);
        assert_eq!(m.spider_suit_count, 1);
        handle_action(&mut m, "spider-suit-hearts");
        assert_eq!(m.spider_one_suit, Suit::Hearts);
        assert_eq!(m.confirm, None);
        let session = m.session.as_ref().unwrap();
        for cid in 9..=18u8 {
            let top = session.piles().get(cid).top().unwrap();
            assert_eq!(top.suit, Suit::Hearts, "deck swapped to Hearts");
        }
    }

    #[test]
    fn options_menu_always_present_even_on_title_screen() {
        let m = AppModel::new();
        assert!(
            m.kind.is_none(),
            "fresh AppModel must start on the title screen"
        );
        let menus = build_menus(&m);
        // Single top-level "Menu" entry with Game/Options/Help as
        // submenus underneath.
        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].label, "Menu");
        let labels = submenu_labels(&menus[0].items);
        assert!(
            labels.contains(&"Options".to_string()),
            "Options submenu should be present on the title screen, got {labels:?}"
        );
    }

    #[test]
    fn debug_submenu_exposes_performance_window_toggle() {
        let mut m = AppModel::new();
        // The Debug submenu hosts a single "Performance Window" entry
        // (no separate Debug Mode gate — opening the window IS the
        // debug intent).  Visible on every screen, including the title.
        let items = options_menu_items(&m, m.kind);
        let debug = find_submenu(&items, "Debug").expect("Debug submenu");
        assert_eq!(
            visible_action_labels(&debug),
            vec!["Performance Window", "Generate Seed Games\u{2026}"]
        );
        assert!(!m.show_performance_window.get());

        // Triggering the Performance Window action flips the model cell.
        handle_action(&mut m, "toggle-performance-window");
        assert!(m.show_performance_window.get());
        handle_action(&mut m, "toggle-performance-window");
        assert!(!m.show_performance_window.get());
    }

    fn submenu_labels(items: &[MenuEntry]) -> Vec<String> {
        items
            .iter()
            .filter_map(|e| match e {
                MenuEntry::Item(it) if it.has_submenu() => Some(it.label.clone()),
                _ => None,
            })
            .collect()
    }

    /// Walk an entry list and return a clone of the nested submenu
    /// whose label matches `title`.  Used by the Debug-menu tests
    /// above to assert what the user sees.
    fn find_submenu(items: &[MenuEntry], title: &str) -> Option<MenuItem> {
        for entry in items {
            if let MenuEntry::Item(item) = entry {
                if item.label == title && item.has_submenu() {
                    return Some(item.clone());
                }
            }
        }
        None
    }

    /// Collect the action-row labels in a submenu (skips separators
    /// and any nested submenus).  Test helper.
    fn visible_action_labels(submenu: &MenuItem) -> Vec<String> {
        submenu
            .submenu
            .iter()
            .filter_map(|e| match e {
                MenuEntry::Item(item) if !item.has_submenu() => Some(item.label.clone()),
                _ => None,
            })
            .collect()
    }
}
