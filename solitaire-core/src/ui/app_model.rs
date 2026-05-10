//! Shared mutable state for every Solitaire widget.

use std::cell::RefCell;
use std::rc::Rc;

use web_time::Instant;

use crate::games::freecell::FreeCell;
use crate::games::klondike::Klondike;
use crate::games::moms::MomsSolitaire;
use crate::games::spider::Spider;
use crate::games::GameKind;
use crate::session::GameSession;

use super::dyn_session::DynGameSession;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Title,
    Game,
    Won,
}

/// Which Help dialog (if any) is currently overlaid. `None` = no dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpKind {
    About,
    Klondike,
    FreeCell,
    Spider,
    MomsSolitaire,
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
    /// Open Help dialog, if any. The `HelpDialog` widget reads this and
    /// paints the corresponding markdown content as a modal overlay.
    pub help: Option<HelpKind>,
}

impl AppModel {
    pub fn new() -> Self {
        Self {
            screen: Screen::Title,
            session: None,
            kind: None,
            toast: None,
            klondike_draw_count: 1,
            help: None,
        }
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
            GameKind::Spider => Box::new(GameSession::new(Spider::four_suit(), seed)),
            GameKind::MomsSolitaire => Box::new(GameSession::new(MomsSolitaire::new(), seed)),
        };
        self.session = Some(session);
        self.kind = Some(kind);
        self.screen = Screen::Game;
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
        if matches!(self.kind, Some(GameKind::Klondike)) {
            self.restart_current_deal();
        }
    }

    pub fn back_to_title(&mut self) {
        self.session = None;
        self.kind = None;
        self.screen = Screen::Title;
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
