//! Shared mutable state for every Solitaire widget.

use std::cell::RefCell;
use std::rc::Rc;

use web_time::Instant;

use crate::games::freecell::FreeCell;
use crate::games::klondike::Klondike;
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

pub struct AppModel {
    pub screen: Screen,
    pub session: Option<Box<dyn DynGameSession>>,
    pub kind: Option<GameKind>,
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

    pub fn start_game(&mut self, kind: GameKind) {
        let seed = wallclock_seed();
        let session: Box<dyn DynGameSession> = match kind {
            GameKind::Klondike => Box::new(GameSession::new(Klondike::new(), seed)),
            GameKind::Classic => Box::new(GameSession::new(Klondike::classic(), seed)),
            GameKind::FreeCell => Box::new(GameSession::new(FreeCell::new(), seed)),
            GameKind::Spider => Box::new(GameSession::new(Spider::four_suit(), seed)),
        };
        self.session = Some(session);
        self.kind = Some(kind);
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
    let now = Instant::now();
    let nanos = now.elapsed().as_nanos() as u64;
    nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xCAFEBABE
}
