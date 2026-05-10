//! Shared mutable state for every Solitaire widget.

use std::cell::RefCell;
use std::rc::Rc;

use web_time::Instant;

use crate::games::klondike::Klondike;
use crate::games::GameKind;
use crate::session::GameSession;

use super::dyn_session::DynGameSession;

/// Top-level screen the app is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Title,
    Game,
    Won,
}

pub struct AppModel {
    pub screen: Screen,
    /// Active game session. `None` while on the title screen.
    pub session: Option<Box<dyn DynGameSession>>,
    /// Which variant the active session is for. Used by the HUD title and
    /// score-write game-slug.
    pub kind: Option<GameKind>,
    /// "Coming soon" toast — shown when the user clicks an unimplemented
    /// title button (FreeCell, Spider, Classic in Phase 2).
    pub toast: Option<(String, Instant)>,
}

impl AppModel {
    pub fn new() -> Self {
        Self {
            screen: Screen::Title,
            session: None,
            kind: None,
            toast: None,
        }
    }

    pub fn start_klondike(&mut self) {
        let seed = wallclock_seed();
        let session: GameSession<Klondike> = GameSession::new(Klondike::new(), seed);
        self.session = Some(Box::new(session));
        self.kind = Some(GameKind::Klondike);
        self.screen = Screen::Game;
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
    // `web-time::Instant` doesn't yield ns directly; use elapsed-since-an
    // arbitrary epoch as the seed source. Combined with std::process::id
    // for a tiny extra perturbation.
    let now = Instant::now();
    let nanos = now.elapsed().as_nanos() as u64;
    nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xCAFEBABE
}
