//! Dyn-trait wrapper around `GameSession<R>` so the UI layer can store
//! any of the four variants behind a single `Box<dyn DynGameSession>`
//! without making `GameWidget` generic.

use crate::games::GameRules;
use crate::piles::{PileId, PileSet};
use crate::session::{GameSession, Move};

pub trait DynGameSession {
    fn try_apply(&mut self, m: Move) -> bool;
    fn try_undo(&mut self) -> bool;
    fn legal_move(&self, m: &Move) -> bool;
    fn on_pile_click(&self, pile: PileId) -> Option<Move>;
    fn auto_complete_step(&self) -> Option<Move>;
    fn piles(&self) -> &PileSet;
    fn is_won(&self) -> bool;
    fn game_slug(&self) -> &'static str;
}

impl<R: GameRules> DynGameSession for GameSession<R> {
    fn try_apply(&mut self, m: Move) -> bool {
        GameSession::try_apply(self, m)
    }
    fn try_undo(&mut self) -> bool {
        GameSession::try_undo(self)
    }
    fn legal_move(&self, m: &Move) -> bool {
        self.rules.legal_move(&self.piles, m)
    }
    fn on_pile_click(&self, pile: PileId) -> Option<Move> {
        self.rules.on_pile_click(&self.piles, pile)
    }
    fn auto_complete_step(&self) -> Option<Move> {
        self.rules.auto_complete_step(&self.piles)
    }
    fn piles(&self) -> &PileSet {
        &self.piles
    }
    fn is_won(&self) -> bool {
        GameSession::is_won(self)
    }
    fn game_slug(&self) -> &'static str {
        self.rules.game_slug()
    }
}
