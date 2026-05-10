//! Session — game state mutation, move application, undo stack.

mod moves;
mod undo;

#[cfg(test)]
mod tests;

pub use moves::{apply_move, revert_move, Move};
pub use undo::UndoStack;

use crate::games::GameRules;
use crate::piles::PileSet;

/// One play session: the rules, the live pile state, and an undo stack.
/// Score and timing live in the UI model, not here — the rules engine is
/// stateless beyond the move history.
pub struct GameSession<R: GameRules> {
    pub rules: R,
    pub piles: PileSet,
    pub undo: UndoStack,
    /// RNG seed used to deal — re-deal-same-game uses this; logged with
    /// score submissions for reproducibility.
    pub seed: u64,
}

impl<R: GameRules> GameSession<R> {
    pub fn new(rules: R, seed: u64) -> Self {
        let slots = rules.pile_layout();
        let mut piles = PileSet::from_slots(slots);
        let mut rng = rand::rngs::StdRng::from_seed_u64(seed);
        rules.deal(&mut piles, &mut rng);
        Self {
            rules,
            piles,
            undo: UndoStack::default(),
            seed,
        }
    }

    /// Try to apply a move. Returns `true` if the move was legal and
    /// applied; `false` if rejected.
    pub fn try_apply(&mut self, m: Move) -> bool {
        if !self.rules.legal_move(&self.piles, &m) {
            return false;
        }
        apply_move(&mut self.piles, &m);
        self.undo.push(m);
        true
    }

    /// Pop the most recent move and revert it. Returns `false` if the
    /// undo stack is empty.
    pub fn try_undo(&mut self) -> bool {
        let Some(m) = self.undo.pop() else {
            return false;
        };
        revert_move(&mut self.piles, &m);
        true
    }

    pub fn is_won(&self) -> bool {
        self.rules.is_won(&self.piles)
    }
}

/// Local extension on `StdRng` so callers can seed without pulling
/// `SeedableRng` into every call site.
trait StdRngFromSeed {
    fn from_seed_u64(seed: u64) -> Self;
}

impl StdRngFromSeed for rand::rngs::StdRng {
    fn from_seed_u64(seed: u64) -> Self {
        use rand::SeedableRng;
        rand::rngs::StdRng::seed_from_u64(seed)
    }
}
