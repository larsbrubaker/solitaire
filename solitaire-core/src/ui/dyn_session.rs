//! Dyn-trait wrapper around `GameSession<R>` so the UI layer can store
//! any of the four variants behind a single `Box<dyn DynGameSession>`
//! without making `GameWidget` generic.

use agg_gui::geometry::Rect;

use crate::games::GameRules;
use crate::piles::{PileId, PileSet};
use crate::session::{AppliedMoveRecord, GameSession, Move};

pub trait DynGameSession {
    fn try_apply(&mut self, m: Move) -> bool;
    fn try_apply_recording(&mut self, m: Move) -> Option<Vec<AppliedMoveRecord>>;
    /// Apply a batch of moves as a single undo unit. Used for stock
    /// dispenses where one click yields N moves but the player
    /// thinks of it as one action.
    fn try_apply_batch(&mut self, moves: Vec<Move>) -> bool;
    fn try_apply_batch_recording(&mut self, moves: Vec<Move>) -> Option<Vec<AppliedMoveRecord>>;
    /// Apply a move bypassing `legal_move`. Used by engine-initiated
    /// actions like Mom's Solitaire's Shuffle that intentionally fall
    /// outside the user-facing move grammar.
    fn apply_forced(&mut self, m: Move);
    fn try_undo(&mut self) -> bool;
    fn legal_move(&self, m: &Move) -> bool;
    fn on_pile_click(&self, pile: PileId) -> Vec<Move>;
    fn single_click_move(&self, pile: PileId, card_idx: usize) -> Option<Move>;
    fn auto_complete_step(&self) -> Option<Move>;
    fn piles(&self) -> &PileSet;
    fn is_won(&self) -> bool;
    fn game_slug(&self) -> &'static str;
    fn seed(&self) -> u64;
    /// Re-run the active rules' `pile_layout` for `rect` and apply
    /// the resulting positions / sizes to the existing piles. Card
    /// stacks are preserved.
    fn relayout(&mut self, rect: Rect);
}

impl<R: GameRules> DynGameSession for GameSession<R> {
    fn try_apply(&mut self, m: Move) -> bool {
        GameSession::try_apply(self, m)
    }
    fn try_apply_recording(&mut self, m: Move) -> Option<Vec<AppliedMoveRecord>> {
        GameSession::try_apply_recording(self, m)
    }
    fn try_apply_batch(&mut self, moves: Vec<Move>) -> bool {
        GameSession::try_apply_batch(self, moves)
    }
    fn try_apply_batch_recording(&mut self, moves: Vec<Move>) -> Option<Vec<AppliedMoveRecord>> {
        GameSession::try_apply_batch_recording(self, moves)
    }
    fn apply_forced(&mut self, m: Move) {
        GameSession::apply_forced(self, m);
    }
    fn try_undo(&mut self) -> bool {
        GameSession::try_undo(self)
    }
    fn legal_move(&self, m: &Move) -> bool {
        self.rules.legal_move(&self.piles, m)
    }
    fn on_pile_click(&self, pile: PileId) -> Vec<Move> {
        self.rules.on_pile_click(&self.piles, pile)
    }
    fn single_click_move(&self, pile: PileId, card_idx: usize) -> Option<Move> {
        self.rules.single_click_move(&self.piles, pile, card_idx)
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
    fn seed(&self) -> u64 {
        self.seed
    }
    fn relayout(&mut self, rect: Rect) {
        GameSession::relayout(self, rect);
    }
}
