//! Shared mutable state for every Solitaire widget.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use agg_gui::geometry::Rect;
use agg_gui::{shared_frame_history, SharedFrameHistory};
use rand::rngs::StdRng;
use rand::SeedableRng;
use web_time::Instant;

use crate::cards::Suit;
use crate::games::freecell::FreeCell;
use crate::games::klondike::Klondike;
use crate::games::moms::MomsSolitaire;
use crate::games::spider::Spider;
use crate::games::GameKind;
use crate::session::GameSession;
use crate::settings::{PerfWindowState, UserSettings};

use super::dyn_session::DynGameSession;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Title,
    Game,
    Won,
}

/// Which Help dialog (if any) is currently overlaid. `None` = no
/// dialog. Both topics are keyed by `GameKind` so the player only
/// ever sees content for the variant they're playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpKind {
    Rules(GameKind),
    About(GameKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    NewDeal(GameKind),
    MainMenu,
}

pub struct AppModel {
    pub screen: Screen,
    pub session: Option<Box<dyn DynGameSession>>,
    pub kind: Option<GameKind>,
    pub toast: Option<(String, Instant)>,
    /// Persisted across sessions and game variants — the user's preferred
    /// Klondike draw count (1 = standard, 3 = Microsoft "Classic"). Read
    /// when starting Klondike; ignored for FreeCell/Spider.
    pub klondike_draw_count: u8,
    /// Spider suit count (1 = beginner, 2 = intermediate, 4 = classic).
    /// Read when starting Spider; ignored otherwise.
    pub spider_suit_count: u8,
    /// Suit used by 1-suit Spider. Ignored for other suit counts.
    pub spider_one_suit: Suit,
    /// Open Help dialog, if any. The `HelpDialog` widget reads this and
    /// paints the corresponding markdown content as a modal overlay.
    pub help: Option<HelpKind>,
    /// Destructive action waiting for user confirmation. The
    /// `ConfirmDialog` widget reads this and executes/cancels it.
    pub confirm: Option<ConfirmAction>,
    /// Mom's Solitaire state: when the player clicks an Ace gap at
    /// column 0, that gap's pile id lands here and the game waits for
    /// the next click to land on a King — that King will be swapped
    /// into the gap. `None` means no king-pickup is in progress.
    pub moms_waiting_king_at: Option<crate::piles::PileId>,
    /// Mom's Solitaire shuffle counter for the active deal. Matches
    /// `m_NumShuffles` in the C# original; surfaced on screen so the
    /// player can see how many shuffles the solve cost.
    pub moms_shuffles: u32,
    /// Whether the Performance window (Mean CPU usage + sparkline) is
    /// currently open.  Held as `Rc<Cell<bool>>` so the agg-gui
    /// `Window` widget that hosts it can wire `with_visible_cell` to
    /// the same backing cell — a click on the window's × button writes
    /// the cell directly, and the menu toggle writes through the
    /// `set_performance_window_open` helper.  Both paths converge on
    /// one source of truth.
    pub show_performance_window: Rc<Cell<bool>>,
    /// Live position + size of the Performance window.  Wired into the
    /// `agg_gui::widgets::Window` via `with_position_cell` so the
    /// widget writes the current bounds back here every layout pass.
    /// `maybe_save_perf_window_settings` reads this to decide whether
    /// the persisted blob needs an update.
    pub perf_window_bounds: Rc<Cell<Rect>>,
    /// Last-persisted snapshot of `(visible, bounds)`.  Used by
    /// `maybe_save_perf_window_settings` to short-circuit when nothing
    /// has changed — keeps `AppRootWidget::paint` from hitting the
    /// disk on every frame.
    last_saved_perf_window: Cell<(bool, Rect)>,
    /// Rolling buffer of recent frame times, fed by the platform shell
    /// (native winit loop or wasm `render` entry point) and read by
    /// the Performance window's `PerformanceView`.  Lives on the
    /// model so the platform shell can grab a clone via
    /// [`SharedModel`] borrow without an extra plumbing layer.
    pub frame_history: SharedFrameHistory,
}

impl AppModel {
    pub fn new() -> Self {
        // Persisted Options-menu choices (Klondike draw count, Spider
        // suit count + one-suit choice) load from the platform's
        // key/value store (`localStorage` in WASM, file-backed on
        // native, in-memory in headless tests). When the backend is
        // absent or the stored blob doesn't parse, fall back to the
        // `UserSettings::default()` values.
        let s = UserSettings::load();
        let perf_bounds = perf_state_to_rect(s.perf_window);
        Self {
            screen: Screen::Title,
            session: None,
            kind: None,
            toast: None,
            klondike_draw_count: s.klondike_draw_count,
            spider_suit_count: s.spider_suit_count,
            spider_one_suit: s.spider_one_suit,
            help: None,
            confirm: None,
            moms_waiting_king_at: None,
            moms_shuffles: 0,
            show_performance_window: Rc::new(Cell::new(s.perf_window.visible)),
            perf_window_bounds: Rc::new(Cell::new(perf_bounds)),
            last_saved_perf_window: Cell::new((s.perf_window.visible, perf_bounds)),
            frame_history: shared_frame_history(),
        }
    }

    /// Snapshot the persisted-settings fields and write them to the
    /// platform store. Called from every setter that touches a
    /// persisted field. Failures are silent (no backend, etc.).
    fn save_settings(&self) {
        let perf_window = rect_to_perf_state(
            self.show_performance_window.get(),
            self.perf_window_bounds.get(),
        );
        // Keep `last_saved_perf_window` in sync so the diff guard in
        // `maybe_save_perf_window_settings` doesn't immediately fire a
        // duplicate write the next time it ticks.
        self.last_saved_perf_window
            .set((perf_window.visible, self.perf_window_bounds.get()));
        UserSettings {
            klondike_draw_count: self.klondike_draw_count,
            spider_suit_count: self.spider_suit_count,
            spider_one_suit: self.spider_one_suit,
            perf_window,
        }
        .save();
    }

    /// Diff-gated persistence for the Debug → Performance window.  The
    /// `agg_gui::widgets::Window` widget rewrites `perf_window_bounds`
    /// every layout pass and the close-button writes
    /// `show_performance_window` directly, so we just compare against
    /// the last-saved snapshot.  Called from `AppRootWidget::paint` so
    /// it runs once per frame the app actually repaints — which, with
    /// the reactive event loop, is exactly when state can have
    /// changed.  No-op when nothing differs (the common case once the
    /// user stops dragging).
    pub fn maybe_save_perf_window_settings(&self) {
        let visible = self.show_performance_window.get();
        let bounds = self.perf_window_bounds.get();
        if self.last_saved_perf_window.get() == (visible, bounds) {
            return;
        }
        self.save_settings();
    }

    /// Open / close the Performance window.  Both the Debug menu's
    /// "Performance Window" toggle and the window's own × button write
    /// through the shared `Rc<Cell<bool>>`, so this setter just keeps
    /// the API symmetric with the other model setters.
    pub fn set_performance_window_open(&mut self, open: bool) {
        if self.show_performance_window.get() == open {
            return;
        }
        self.show_performance_window.set(open);
        // Persist the visibility change immediately — the window may
        // not get a layout pass before the user closes the app, so
        // waiting for the AppRoot tick would lose the toggle.
        self.save_settings();
    }

    pub fn start_game(&mut self, kind: GameKind) {
        self.start_game_with_seed(kind, wallclock_seed());
    }

    pub fn request_new_deal(&mut self, kind: GameKind) {
        if self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::NewDeal(kind));
        } else {
            self.start_game(kind);
        }
    }

    pub fn request_main_menu(&mut self) {
        if self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::MainMenu);
        } else {
            self.back_to_title();
        }
    }

    pub fn confirm_pending_action(&mut self) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::NewDeal(kind) => self.start_game(kind),
            ConfirmAction::MainMenu => self.back_to_title(),
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.confirm = None;
    }

    fn game_in_progress_has_moves(&self) -> bool {
        self.screen == Screen::Game
            && self
                .session
                .as_ref()
                .is_some_and(|session| session.has_moves())
    }

    pub fn start_game_with_seed(&mut self, kind: GameKind, seed: u64) {
        let session: Box<dyn DynGameSession> = match kind {
            GameKind::Klondike => Box::new(GameSession::new(
                Klondike::with_draw_count(self.klondike_draw_count),
                seed,
            )),
            GameKind::FreeCell => Box::new(GameSession::new(FreeCell::new(), seed)),
            GameKind::Spider => Box::new(GameSession::new(
                Spider {
                    suit_count: self.spider_suit_count,
                    one_suit: self.spider_one_suit,
                },
                seed,
            )),
            GameKind::MomsSolitaire => Box::new(GameSession::new(MomsSolitaire::new(), seed)),
        };
        self.session = Some(session);
        self.kind = Some(kind);
        self.screen = Screen::Game;
        self.confirm = None;
        // Any Mom's-specific UI state is per-game; reset so a new
        // Klondike doesn't inherit a stale "waiting for king" or
        // shuffle count.
        self.moms_waiting_king_at = None;
        self.moms_shuffles = 0;
    }

    /// Restart the current deal — same kind, same seed, fresh shuffle.
    pub fn restart_current_deal(&mut self) {
        let Some(kind) = self.kind else { return };
        let Some(seed) = self.session.as_ref().map(|s| s.seed()) else {
            return;
        };
        self.start_game_with_seed(kind, seed);
    }

    /// Apply a new Klondike draw count. If a Klondike game is in progress,
    /// re-deal it with the same seed under the new rules so the user sees
    /// the change immediately.
    pub fn set_klondike_draw_count(&mut self, n: u8) {
        if self.klondike_draw_count == n {
            return;
        }
        self.klondike_draw_count = n;
        self.save_settings();
        if matches!(self.kind, Some(GameKind::Klondike)) {
            self.restart_current_deal();
        }
    }

    /// Apply a new Spider suit count (1 / 2 / 4). Same restart-with-same-
    /// seed semantics as `set_klondike_draw_count`.
    pub fn set_spider_suit_count(&mut self, n: u8) {
        if self.spider_suit_count == n {
            return;
        }
        self.spider_suit_count = n;
        self.save_settings();
        if matches!(self.kind, Some(GameKind::Spider)) {
            self.restart_current_deal();
        }
    }

    /// Apply a new active suit for 1-suit Spider. No-op (other than
    /// the model field update) when the active game isn't 1-suit
    /// Spider, since no on-screen card would change.
    pub fn set_spider_one_suit(&mut self, suit: Suit) {
        if self.spider_one_suit == suit {
            return;
        }
        self.spider_one_suit = suit;
        self.save_settings();
        if matches!(self.kind, Some(GameKind::Spider)) && self.spider_suit_count == 1 {
            self.restart_current_deal();
        }
    }

    pub fn back_to_title(&mut self) {
        self.session = None;
        self.kind = None;
        self.screen = Screen::Title;
        self.confirm = None;
        self.moms_waiting_king_at = None;
        self.moms_shuffles = 0;
    }

    /// Mom's Solitaire: shuffle the out-of-order cells in place,
    /// increment the on-screen shuffle counter, and clear any pending
    /// king-pickup. No-op on any other variant. Returns `true` if at
    /// least one swap was performed.
    pub fn try_moms_shuffle(&mut self) -> bool {
        if !matches!(self.kind, Some(GameKind::MomsSolitaire)) {
            return false;
        }
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        let mut rng = StdRng::seed_from_u64(wallclock_seed());
        let swaps = crate::games::moms::compute_shuffle_swaps(session.piles(), &mut rng);
        if swaps.is_empty() {
            return false;
        }
        // Shuffle swaps never satisfy Mom's user-facing `legal_move`
        // (which requires the destination to be an Ace gap matching
        // its left neighbour). Use the unchecked path; the swaps
        // still land on the undo stack.
        for (a, b) in swaps {
            session.apply_forced(crate::session::Move::swap(a, b));
        }
        self.moms_shuffles += 1;
        self.moms_waiting_king_at = None;
        true
    }

    pub fn show_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    /// Drop a stale toast (older than [`TOAST_LIFETIME`]).
    pub fn tick_toast(&mut self) {
        if let Some((_, started)) = &self.toast {
            if started.elapsed() > TOAST_LIFETIME {
                self.toast = None;
            }
        }
    }
}

/// How long a toast banner stays visible before [`AppModel::tick_toast`]
/// drops it.  Exported so widgets that want to schedule a wake-up at
/// the toast expiry (`next_draw_deadline`) can reuse the same value
/// without forking a magic number.
pub const TOAST_LIFETIME: std::time::Duration = std::time::Duration::from_secs(3);

impl Default for AppModel {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedModel = Rc<RefCell<AppModel>>;

pub fn shared_model() -> SharedModel {
    Rc::new(RefCell::new(AppModel::new()))
}

fn wallclock_seed() -> u64 {
    let now = Instant::now();
    let nanos = now.elapsed().as_nanos() as u64;
    nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xCAFEBABE
}

/// Convert the persisted flat-record form to the `Rect` the agg-gui
/// `Window` widget consumes (and writes back into `position_cell`).
fn perf_state_to_rect(s: PerfWindowState) -> Rect {
    Rect::new(s.x, s.y, s.width, s.height)
}

/// Inverse of [`perf_state_to_rect`].  Drops `bounds.width / height`
/// to a sane floor so a zeroed cell (the value before the Window's
/// first layout pass) doesn't silently overwrite a saved valid size.
fn rect_to_perf_state(visible: bool, bounds: Rect) -> PerfWindowState {
    PerfWindowState {
        visible,
        x: bounds.x,
        y: bounds.y,
        width: bounds.width.max(120.0),
        height: bounds.height.max(80.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    struct StorageGuard {
        _store: Rc<RefCell<HashMap<String, String>>>,
    }

    impl Drop for StorageGuard {
        fn drop(&mut self) {
            crate::platform::clear_storage_io_for_test();
        }
    }

    fn install_test_storage() -> StorageGuard {
        let store = Rc::new(RefCell::new(HashMap::new()));
        let load_store = store.clone();
        let save_store = store.clone();
        crate::platform::set_storage_io(
            move |k| load_store.borrow().get(k).cloned(),
            move |k, v| {
                save_store.borrow_mut().insert(k.to_string(), v.to_string());
            },
        );
        StorageGuard { _store: store }
    }

    #[test]
    fn app_model_loads_persisted_spider_options() {
        let _guard = install_test_storage();
        UserSettings {
            klondike_draw_count: 3,
            spider_suit_count: 1,
            spider_one_suit: Suit::Spades,
            perf_window: PerfWindowState::default(),
        }
        .save();

        let mut model = AppModel::new();
        assert_eq!(model.klondike_draw_count, 3);
        assert_eq!(model.spider_suit_count, 1);
        assert_eq!(model.spider_one_suit, Suit::Spades);

        model.start_game_with_seed(GameKind::Spider, 7);
        let session = model.session.as_ref().unwrap();
        for cid in 9..=18u8 {
            let top = session.piles().get(cid).top().unwrap();
            assert_eq!(top.suit, Suit::Spades);
        }
    }

    #[test]
    fn perf_window_state_round_trips_through_settings() {
        let _guard = install_test_storage();

        // Seed the settings store with a non-default perf window
        // layout (open + offset + resized).
        UserSettings {
            klondike_draw_count: 1,
            spider_suit_count: 1,
            spider_one_suit: Suit::Spades,
            perf_window: PerfWindowState {
                visible: true,
                x: 240.0,
                y: 180.0,
                width: 480.0,
                height: 240.0,
            },
        }
        .save();

        // First model load must surface the saved perf window.
        let model = AppModel::new();
        assert!(model.show_performance_window.get());
        assert_eq!(
            model.perf_window_bounds.get(),
            Rect::new(240.0, 180.0, 480.0, 240.0)
        );

        // Simulate the agg-gui Window writing fresh bounds back into
        // the position cell after the user dragged the window — the
        // model should detect the diff and persist on the next tick.
        model
            .perf_window_bounds
            .set(Rect::new(300.0, 220.0, 480.0, 240.0));
        model.maybe_save_perf_window_settings();
        let reloaded = UserSettings::load().perf_window;
        assert_eq!(reloaded.x, 300.0);
        assert_eq!(reloaded.y, 220.0);
        assert!(reloaded.visible);

        // Subsequent ticks with no further changes are no-ops (the
        // diff guard short-circuits before touching the storage
        // backend) — verify by re-reading the same blob.
        model.maybe_save_perf_window_settings();
        let again = UserSettings::load().perf_window;
        assert_eq!(again, reloaded);
    }

    #[test]
    fn new_deal_request_on_fresh_game_skips_confirmation() {
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 123);
        let original_seed = model.session.as_ref().unwrap().seed();

        model.request_new_deal(GameKind::Spider);

        assert_eq!(model.confirm, None);
        assert_eq!(model.screen, Screen::Game);
        assert_ne!(model.session.as_ref().unwrap().seed(), original_seed);
    }

    #[test]
    fn new_deal_request_after_move_requires_confirmation() {
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 123);
        let original_seed = model.session.as_ref().unwrap().seed();
        apply_spider_stock_deal(&mut model);

        model.request_new_deal(GameKind::Spider);

        assert_eq!(
            model.confirm,
            Some(ConfirmAction::NewDeal(GameKind::Spider))
        );
        assert_eq!(model.session.as_ref().unwrap().seed(), original_seed);

        model.confirm_pending_action();

        assert_eq!(model.confirm, None);
        assert_eq!(model.screen, Screen::Game);
        assert_ne!(model.session.as_ref().unwrap().seed(), original_seed);
    }

    #[test]
    fn main_menu_request_on_fresh_game_skips_confirmation() {
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 456);

        model.request_main_menu();

        assert_eq!(model.confirm, None);
        assert_eq!(model.screen, Screen::Title);
        assert!(model.session.is_none());
    }

    #[test]
    fn main_menu_request_after_move_requires_confirmation() {
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 456);
        apply_spider_stock_deal(&mut model);

        model.request_main_menu();

        assert_eq!(model.confirm, Some(ConfirmAction::MainMenu));
        assert_eq!(model.screen, Screen::Game);
        assert!(model.session.is_some());

        model.cancel_pending_action();
        assert_eq!(model.confirm, None);
        assert_eq!(model.screen, Screen::Game);

        model.request_main_menu();
        model.confirm_pending_action();
        assert_eq!(model.screen, Screen::Title);
        assert!(model.session.is_none());
    }

    fn apply_spider_stock_deal(model: &mut AppModel) {
        let session = model.session.as_mut().expect("test starts a game");
        let moves = session.on_pile_click(8);
        assert!(!moves.is_empty(), "fresh Spider stock should deal");
        assert!(session.try_apply_batch(moves));
        assert!(session.has_moves());
    }
}
