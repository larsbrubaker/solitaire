//! Shared mutable state for every Solitaire widget.

use std::cell::RefCell;
use std::rc::Rc;

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
use crate::settings::UserSettings;

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
    /// Mom's Solitaire state: when the player clicks an Ace gap at
    /// column 0, that gap's pile id lands here and the game waits for
    /// the next click to land on a King — that King will be swapped
    /// into the gap. `None` means no king-pickup is in progress.
    pub moms_waiting_king_at: Option<crate::piles::PileId>,
    /// Mom's Solitaire shuffle counter for the active deal. Matches
    /// `m_NumShuffles` in the C# original; surfaced on screen so the
    /// player can see how many shuffles the solve cost.
    pub moms_shuffles: u32,
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
        Self {
            screen: Screen::Title,
            session: None,
            kind: None,
            toast: None,
            klondike_draw_count: s.klondike_draw_count,
            spider_suit_count: s.spider_suit_count,
            spider_one_suit: s.spider_one_suit,
            help: None,
            moms_waiting_king_at: None,
            moms_shuffles: 0,
        }
    }

    /// Snapshot the persisted-settings fields and write them to the
    /// platform store. Called from every setter that touches a
    /// persisted field. Failures are silent (no backend, etc.).
    fn save_settings(&self) {
        UserSettings {
            klondike_draw_count: self.klondike_draw_count,
            spider_suit_count: self.spider_suit_count,
            spider_one_suit: self.spider_one_suit,
        }
        .save();
    }

    pub fn start_game(&mut self, kind: GameKind) {
        self.start_game_with_seed(kind, wallclock_seed());
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

    /// Drop a stale toast (older than 3 s).
    pub fn tick_toast(&mut self) {
        if let Some((_, started)) = &self.toast {
            if started.elapsed().as_secs_f64() > 3.0 {
                self.toast = None;
            }
        }
    }
}

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
