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
use crate::games::winnable_seeds::MS_FREECELL_MAX;

/// Cached widened form of `MS_FREECELL_MAX` for u64 comparisons.
const MS_FREECELL_MAX_U64: u64 = MS_FREECELL_MAX as u64;
use crate::games::klondike::Klondike;
use crate::games::moms::MomsSolitaire;
use crate::games::spider::{best_spider_hint, Spider, SpiderHint};
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
/// dialog. The `About` / `Rules` variants are per-game; `AboutSuite`
/// is the company-and-suite write-up shown on the title screen and
/// underneath the per-game About when a session is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpKind {
    Rules(GameKind),
    About(GameKind),
    AboutSuite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    NewDeal(GameKind),
    MainMenu,
    /// Klondike draw-count change picked from Options while a game
    /// with moves is in progress. The pending value rides on the
    /// variant; only applied if the user confirms.
    ApplyKlondikeDrawCount(u8),
    /// Spider suit-count change picked while a game with moves is
    /// in progress.
    ApplySpiderSuitCount(u8),
    /// Spider 1-suit choice change picked while a game with moves is
    /// in progress.
    ApplySpiderOneSuit(Suit),
    /// "Winnable deals only" toggle for Spider picked while a game
    /// with moves is in progress.
    ApplySpiderWinnableOnly(bool),
    /// Toggle for FreeCell's "Winnable deals only".
    ApplyFreeCellWinnableOnly(bool),
    /// Toggle for Klondike's "Winnable deals only".
    ApplyKlondikeWinnableOnly(bool),
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
    /// When true, Spider new-deal picks a seed from the bundled
    /// solver-verified winnable list instead of a wallclock seed.
    pub spider_winnable_only: bool,
    /// When true, FreeCell new-deal pulls from the Microsoft
    /// 32,000-deal pool (skipping the known-unwinnable #11982).
    pub freecell_winnable_only: bool,
    /// When true, Klondike new-deal picks a seed from the bundled
    /// solver-verified winnable list instead of a wallclock seed.
    pub klondike_winnable_only: bool,
    /// Open Help dialog, if any. The `HelpDialog` widget reads this and
    /// paints the corresponding markdown content as a modal overlay.
    pub help: Option<HelpKind>,
    /// Destructive action waiting for user confirmation. The
    /// `ConfirmDialog` widget reads this and executes/cancels it.
    pub confirm: Option<ConfirmAction>,
    /// True while the "Play deal number…" modal is open. The dialog
    /// widget reads this for visibility and writes it on cancel /
    /// commit.
    pub play_deal_dialog_open: bool,
    /// Mom's Solitaire state: when the player clicks an Ace gap at
    /// column 0, that gap's pile id lands here and the game waits for
    /// the next click to land on a King — that King will be swapped
    /// into the gap. `None` means no king-pickup is in progress.
    pub moms_waiting_king_at: Option<crate::piles::PileId>,
    /// Mom's Solitaire shuffle counter for the active deal. Matches
    /// `m_NumShuffles` in the C# original; surfaced on screen so the
    /// player can see how many shuffles the solve cost.
    pub moms_shuffles: u32,
    /// Spider-only: most recent hint produced by the Hint button.
    /// `None` when no hint is active. Cleared on every move/undo and
    /// when the active game changes.
    pub spider_hint: Option<SpiderHint>,
    /// Monotonic counter bumped on every Hint button press. The
    /// `GameWidget` tracks the last-seen value and re-plays the ghost
    /// preview animation whenever this changes, so a second press
    /// with the same recommended move still replays the slide.
    pub spider_hint_seq: u64,
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
            spider_winnable_only: s.spider_winnable_only,
            freecell_winnable_only: s.freecell_winnable_only,
            klondike_winnable_only: s.klondike_winnable_only,
            help: None,
            confirm: None,
            play_deal_dialog_open: false,
            moms_waiting_king_at: None,
            moms_shuffles: 0,
            spider_hint: None,
            spider_hint_seq: 0,
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
            spider_winnable_only: self.spider_winnable_only,
            freecell_winnable_only: self.freecell_winnable_only,
            klondike_winnable_only: self.klondike_winnable_only,
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
        // Route through the winnable-only picker when the player has
        // the toggle on for this variant. The picker re-uses the
        // wallclock seed as its own RNG state so each "New Deal"
        // still produces a different pick from the list.
        let seed = wallclock_seed();
        let resolved = match kind {
            GameKind::Spider if self.spider_winnable_only => {
                crate::games::winnable_seeds::pick_spider_winnable(seed)
            }
            GameKind::Klondike if self.klondike_winnable_only => {
                crate::games::winnable_seeds::pick_klondike_winnable(seed)
            }
            GameKind::FreeCell if self.freecell_winnable_only => {
                // Microsoft FreeCell numbering: encode the game
                // number (1..32_000, skipping #11982) as the seed so
                // Restart re-deals the same game.
                let n = crate::games::winnable_seeds::pick_ms_freecell_winnable(seed);
                n as u64
            }
            _ => seed,
        };
        self.start_game_with_seed(kind, resolved);
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
            ConfirmAction::ApplyKlondikeDrawCount(n) => {
                self.klondike_draw_count = n;
                self.save_settings();
                self.restart_current_deal();
            }
            ConfirmAction::ApplySpiderSuitCount(n) => {
                self.spider_suit_count = n;
                self.save_settings();
                self.restart_current_deal();
            }
            ConfirmAction::ApplySpiderOneSuit(suit) => {
                self.spider_one_suit = suit;
                self.save_settings();
                self.restart_current_deal();
            }
            ConfirmAction::ApplySpiderWinnableOnly(on) => {
                self.spider_winnable_only = on;
                self.save_settings();
                self.start_game(GameKind::Spider);
            }
            ConfirmAction::ApplyFreeCellWinnableOnly(on) => {
                self.freecell_winnable_only = on;
                self.save_settings();
                self.start_game(GameKind::FreeCell);
            }
            ConfirmAction::ApplyKlondikeWinnableOnly(on) => {
                self.klondike_winnable_only = on;
                self.save_settings();
                self.start_game(GameKind::Klondike);
            }
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.confirm = None;
    }

    /// Open the "Play deal number…" modal. No-op when no game is
    /// active (the user reaches it from the Game menu).
    pub fn open_play_deal_dialog(&mut self) {
        if self.session.is_some() {
            self.play_deal_dialog_open = true;
        }
    }

    pub fn cancel_play_deal_dialog(&mut self) {
        self.play_deal_dialog_open = false;
    }

    /// Parse + apply a deal-number string. Returns `Ok(())` on a
    /// successful jump (dialog closes), `Err(msg)` to display in
    /// the dialog when the input doesn't fit the active variant.
    pub fn commit_play_deal_dialog(&mut self, input: &str) -> Result<(), &'static str> {
        let Some(kind) = self.kind else {
            return Err("No active game");
        };
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("Enter a deal number");
        }
        let seed = parse_deal_input(trimmed).ok_or("Couldn't read that number")?;
        if matches!(kind, GameKind::FreeCell) && self.freecell_winnable_only {
            if !(1..=MS_FREECELL_MAX_U64).contains(&seed) {
                return Err("Game number out of range");
            }
            if seed <= u32::MAX as u64
                && crate::games::winnable_seeds::is_ms_freecell_unwinnable(seed as u32)
            {
                return Err("That game number is on Microsoft's unwinnable list");
            }
        }
        self.play_deal_dialog_open = false;
        self.confirm = None;
        self.start_game_with_seed(kind, seed);
        Ok(())
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
            GameKind::FreeCell => {
                // When the winnable-only toggle is on, `start_game`
                // hands us a Microsoft game number (1..32_000) cast
                // into a u64. Build the FreeCell rules with that
                // number so the deal reproduces Jim Horne's classic
                // layout. Restart-with-same-seed then re-deals the
                // exact same Microsoft game.
                if self.freecell_winnable_only && seed > 0 && seed <= MS_FREECELL_MAX_U64 {
                    Box::new(GameSession::new(
                        FreeCell::with_ms_game_number(seed as u32),
                        seed,
                    ))
                } else {
                    Box::new(GameSession::new(FreeCell::new(), seed))
                }
            }
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
        self.spider_hint = None;
    }

    /// Restart the current deal — same kind, same seed, fresh shuffle.
    pub fn restart_current_deal(&mut self) {
        let Some(kind) = self.kind else { return };
        let Some(seed) = self.session.as_ref().map(|s| s.seed()) else {
            return;
        };
        self.start_game_with_seed(kind, seed);
    }

    /// Apply a new Klondike draw count. When the change requires
    /// re-dealing an in-progress Klondike game with moves, the
    /// setting is held until the user confirms via the confirm
    /// dialog — that keeps the player from losing progress to an
    /// accidental menu click. Otherwise we apply (and re-deal) right
    /// away so the visible game reflects the new rules.
    pub fn set_klondike_draw_count(&mut self, n: u8) {
        if self.klondike_draw_count == n {
            return;
        }
        let active = matches!(self.kind, Some(GameKind::Klondike));
        if active && self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::ApplyKlondikeDrawCount(n));
            return;
        }
        self.klondike_draw_count = n;
        self.save_settings();
        if active {
            self.restart_current_deal();
        }
    }

    /// Apply a new Spider suit count (1 / 2 / 4). Confirm-on-progress
    /// behaviour matches `set_klondike_draw_count`.
    pub fn set_spider_suit_count(&mut self, n: u8) {
        if self.spider_suit_count == n {
            return;
        }
        let active = matches!(self.kind, Some(GameKind::Spider));
        if active && self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::ApplySpiderSuitCount(n));
            return;
        }
        self.spider_suit_count = n;
        self.save_settings();
        if active {
            self.restart_current_deal();
        }
    }

    /// Apply a new active suit for 1-suit Spider. Only re-deals when
    /// the active variant is Spider in 1-suit mode (any other state
    /// just persists the setting for the next deal). Confirms first
    /// when the visible Spider game has moves.
    /// Toggle "Winnable deals only" for Spider. Same confirm-on-
    /// progress flow as the other Spider settings: a fresh deal
    /// re-draws immediately from the new pool, mid-game flips queue
    /// a confirm.
    pub fn set_spider_winnable_only(&mut self, on: bool) {
        if self.spider_winnable_only == on {
            return;
        }
        let active = matches!(self.kind, Some(GameKind::Spider));
        if active && self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::ApplySpiderWinnableOnly(on));
            return;
        }
        self.spider_winnable_only = on;
        self.save_settings();
        if active {
            self.start_game(GameKind::Spider);
        }
    }

    /// Toggle "Winnable deals only" for FreeCell.
    pub fn set_freecell_winnable_only(&mut self, on: bool) {
        if self.freecell_winnable_only == on {
            return;
        }
        let active = matches!(self.kind, Some(GameKind::FreeCell));
        if active && self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::ApplyFreeCellWinnableOnly(on));
            return;
        }
        self.freecell_winnable_only = on;
        self.save_settings();
        if active {
            self.start_game(GameKind::FreeCell);
        }
    }

    /// Toggle "Winnable deals only" for Klondike.
    pub fn set_klondike_winnable_only(&mut self, on: bool) {
        if self.klondike_winnable_only == on {
            return;
        }
        let active = matches!(self.kind, Some(GameKind::Klondike));
        if active && self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::ApplyKlondikeWinnableOnly(on));
            return;
        }
        self.klondike_winnable_only = on;
        self.save_settings();
        if active {
            self.start_game(GameKind::Klondike);
        }
    }

    pub fn set_spider_one_suit(&mut self, suit: Suit) {
        if self.spider_one_suit == suit {
            return;
        }
        let active_one_suit_spider =
            matches!(self.kind, Some(GameKind::Spider)) && self.spider_suit_count == 1;
        if active_one_suit_spider && self.game_in_progress_has_moves() {
            self.confirm = Some(ConfirmAction::ApplySpiderOneSuit(suit));
            return;
        }
        self.spider_one_suit = suit;
        self.save_settings();
        if active_one_suit_spider {
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
        self.spider_hint = None;
    }

    /// Compute and store a Spider hint for the active board. No-op when
    /// the active game isn't Spider.
    pub fn show_spider_hint(&mut self) {
        let is_spider = self
            .session
            .as_ref()
            .map(|s| s.game_slug() == "spider")
            .unwrap_or(false);
        if !is_spider {
            return;
        }
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let hint = best_spider_hint(session.piles());
        match hint {
            Some(SpiderHint::Move { .. }) => {
                self.spider_hint = hint;
                // Bump even when the recommended move is unchanged so
                // the GameWidget re-plays the ghost preview animation.
                self.spider_hint_seq = self.spider_hint_seq.wrapping_add(1);
            }
            Some(SpiderHint::StockDeal { .. }) => {
                // The yellow rect on the stock pile is too quiet on
                // its own — players read "ring around a pile" as a
                // valid drop target, not as a next-action prompt.
                // Pair it with a toast so the recommendation is
                // unambiguous.
                self.spider_hint = hint;
                self.spider_hint_seq = self.spider_hint_seq.wrapping_add(1);
                self.show_toast("Deal more cards");
            }
            None => {
                // No legal cascade move and no legal stock deal — show
                // a toast so the user sees feedback for the Hint press
                // rather than silence.
                self.spider_hint = None;
                self.show_toast("No moves");
            }
        }
    }

    /// Drop any pending Spider hint. Called by every move/undo path so
    /// the highlight never lingers past the board state it was computed
    /// for.
    pub fn clear_spider_hint(&mut self) {
        self.spider_hint = None;
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

/// Parse a deal-number user input — accepts plain decimal
/// (`12345`) or hex with the `0x` prefix (`0xdeadbeef`). Used by
/// the Play-deal-number dialog and the seed picker.
fn parse_deal_input(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("0x").or(trimmed.strip_prefix("0X")) {
        u64::from_str_radix(rest, 16).ok()
    } else {
        trimmed.parse::<u64>().ok()
    }
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
            spider_winnable_only: false,
            freecell_winnable_only: false,
            klondike_winnable_only: false,
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
            spider_winnable_only: false,
            freecell_winnable_only: false,
            klondike_winnable_only: false,
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

    #[test]
    fn show_spider_hint_does_not_mutate_session() {
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 123);

        let snapshot: Vec<Vec<crate::cards::Card>> = model
            .session
            .as_ref()
            .unwrap()
            .piles()
            .iter()
            .map(|p| p.cards.clone())
            .collect();

        model.show_spider_hint();

        let after: Vec<Vec<crate::cards::Card>> = model
            .session
            .as_ref()
            .unwrap()
            .piles()
            .iter()
            .map(|p| p.cards.clone())
            .collect();
        assert_eq!(snapshot, after, "Hint must not mutate piles");
        assert!(
            model.spider_hint.is_some(),
            "Fresh Spider deal always has at least the stock deal hint"
        );
    }

    #[test]
    fn show_spider_hint_sets_no_moves_toast_on_dead_board() {
        // Mirrors the user-reported wedge: every legal cascade move
        // is sterile (duplicate-parent shuffle or a wholesale
        // relocation to an empty cascade) and the stock is empty.
        // The Hint button must drop a "No moves" toast on the model
        // instead of leaving the highlight on a misleading move.
        use crate::cards::{Card, Rank, Suit};
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 7);
        // Wipe the dealt state and rebuild the dead board by hand.
        {
            let session = model.session.as_mut().unwrap();
            let piles = session.piles_mut();
            for p in piles.iter_mut() {
                p.cards.clear();
            }
            for r in [Rank::Queen, Rank::Jack] {
                piles
                    .get_mut(9)
                    .cards
                    .push(Card::new(Suit::Spades, r).face_up());
            }
            for r in [
                Rank::Queen,
                Rank::Jack,
                Rank::Ten,
                Rank::Nine,
                Rank::Eight,
                Rank::Seven,
                Rank::Six,
                Rank::Five,
                Rank::Four,
                Rank::Three,
                Rank::Two,
                Rank::Ace,
            ] {
                piles
                    .get_mut(10)
                    .cards
                    .push(Card::new(Suit::Spades, r).face_up());
            }
            piles
                .get_mut(11)
                .cards
                .push(Card::new(Suit::Spades, Rank::Two).face_up());
        }

        model.show_spider_hint();

        assert!(
            model.spider_hint.is_none(),
            "dead board must not surface a hint"
        );
        assert!(
            model
                .toast
                .as_ref()
                .is_some_and(|(msg, _)| msg.contains("No moves")),
            "expected `No moves` toast, got {:?}",
            model.toast.as_ref().map(|(m, _)| m.clone())
        );
    }

    #[test]
    fn show_spider_hint_sets_deal_toast_when_only_stock_left() {
        // User reported a board where every tableau move is sterile
        // but stock still has cards waiting to deal. The Hint button
        // must surface a StockDeal recommendation AND a "Deal more
        // cards" toast so the player understands the next action.
        use crate::cards::{Card, Rank, Suit};
        use crate::games::spider::SpiderHint;
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Spider, 11);
        {
            let session = model.session.as_mut().unwrap();
            let piles = session.piles_mut();
            for p in piles.iter_mut() {
                p.cards.clear();
            }
            // 10 cascades, each topped with a single King — Kings can
            // only land on empty cascades, and none are empty, so the
            // ranker has no tableau candidate. The stock still has
            // cards to deal, so the right hint is `StockDeal`.
            for i in 0..10u8 {
                piles
                    .get_mut(9 + i)
                    .cards
                    .push(Card::new(Suit::Spades, Rank::King).face_up());
            }
            // Stock has at least 10 cards so a deal is legal.
            for _ in 0..10 {
                piles
                    .get_mut(8)
                    .cards
                    .push(Card::new(Suit::Spades, Rank::Two));
            }
        }

        model.show_spider_hint();

        assert!(matches!(
            model.spider_hint,
            Some(SpiderHint::StockDeal { .. })
        ));
        assert!(
            model
                .toast
                .as_ref()
                .is_some_and(|(msg, _)| msg.contains("Deal")),
            "expected `Deal more cards` toast, got {:?}",
            model.toast.as_ref().map(|(m, _)| m.clone())
        );
    }

    #[test]
    fn show_spider_hint_no_op_for_non_spider_games() {
        let _guard = install_test_storage();
        let mut model = AppModel::new();
        model.start_game_with_seed(GameKind::Klondike, 1);
        model.show_spider_hint();
        assert!(model.spider_hint.is_none());
    }

    fn apply_spider_stock_deal(model: &mut AppModel) {
        let session = model.session.as_mut().expect("test starts a game");
        let moves = session.on_pile_click(8);
        assert!(!moves.is_empty(), "fresh Spider stock should deal");
        assert!(session.try_apply_batch(moves));
        assert!(session.has_moves());
    }
}
