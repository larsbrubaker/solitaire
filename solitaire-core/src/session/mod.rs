//! Session — game state mutation, move application, undo stack.

mod moves;
mod undo;

#[cfg(test)]
mod tests;

pub use moves::{apply_move, revert_move, Move};
pub use undo::UndoStack;

use agg_gui::geometry::Rect;

use crate::games::GameRules;
use crate::piles::PileSet;

/// Reference playfield rect used by `GameSession::new` when there's no
/// real viewport yet (headless tests, app startup before first paint).
/// `GameWidget` immediately re-applies `rules.pile_layout(real_rect)`
/// via [`GameSession::relayout`] on the first paint, so this only
/// affects test setups.
pub const DEFAULT_PLAYFIELD_RECT: Rect = Rect::new(0.0, 0.0, 1024.0, 720.0);

/// One play session: the rules, the live pile state, and an undo stack.
pub struct GameSession<R: GameRules> {
    pub rules: R,
    pub piles: PileSet,
    pub undo: UndoStack,
    pub seed: u64,
}

impl<R: GameRules> GameSession<R> {
    pub fn new(rules: R, seed: u64) -> Self {
        let slots = rules.pile_layout(DEFAULT_PLAYFIELD_RECT);
        let mut piles = PileSet::from_slots(&slots);
        let mut rng = StdRngFromSeed::from_seed_u64(seed);
        rules.deal(&mut piles, &mut rng);
        Self {
            rules,
            piles,
            undo: UndoStack::default(),
            seed,
        }
    }

    /// Re-apply the rules' pile layout for a new playfield rect (window
    /// resize, sidebar/standard chrome flip). Pile card stacks are
    /// preserved; only positions, sizes, fan config, and per-pile
    /// rendering flags update.
    pub fn relayout(&mut self, rect: Rect) {
        let slots = self.rules.pile_layout(rect);
        self.piles.update_layout(&slots);
    }

    /// Try to apply a player-initiated move. After success, runs the
    /// rules engine's `after_move` hook in a loop so auto follow-ups
    /// (Spider's K-to-A run collapse, future polish) chain transparently.
    pub fn try_apply(&mut self, m: Move) -> bool {
        if !self.rules.legal_move(&self.piles, &m) {
            return false;
        }
        apply_move(&mut self.piles, &m);
        self.undo.push_user(m);
        while let Some(am) = self.rules.after_move(&self.piles) {
            apply_move(&mut self.piles, &am);
            self.undo.push_auto(am);
        }
        true
    }

    /// Apply a batch of player-initiated moves as a single undo unit.
    /// Used for Spider's stock click (dispenses one card to each of
    /// 10 cascades) and Klondike's stock recycle — operations that
    /// the player thinks of as one action even though they decompose
    /// into multiple `Move`s. Only the FIRST move counts as a user
    /// step on the undo stack; the rest are auto follow-ups so a
    /// single Undo reverts the entire batch.
    pub fn try_apply_batch(&mut self, moves: Vec<Move>) -> bool {
        let mut iter = moves.into_iter();
        let Some(first) = iter.next() else {
            return false;
        };
        if !self.rules.legal_move(&self.piles, &first) {
            return false;
        }
        apply_move(&mut self.piles, &first);
        self.undo.push_user(first);
        for m in iter {
            if !self.rules.legal_move(&self.piles, &m) {
                break;
            }
            apply_move(&mut self.piles, &m);
            self.undo.push_auto(m);
        }
        while let Some(am) = self.rules.after_move(&self.piles) {
            apply_move(&mut self.piles, &am);
            self.undo.push_auto(am);
        }
        true
    }

    /// Apply a move WITHOUT consulting `rules.legal_move`. Used for
    /// engine-initiated state changes that intentionally don't fit
    /// the user-facing move grammar — e.g. Mom's Solitaire's
    /// `Shuffle` action, which swaps out-of-place cells around the
    /// board without satisfying the "click a gap" legality check.
    /// The move still lands on the undo stack so the user can roll
    /// it back like any other.
    pub fn apply_forced(&mut self, m: Move) {
        apply_move(&mut self.piles, &m);
        self.undo.push_user(m);
    }

    /// Pop the most recent USER move (and any auto follow-ups stacked
    /// after it) and revert in reverse order. Returns `false` if the
    /// undo stack is empty.
    pub fn try_undo(&mut self) -> bool {
        let mut undone_anything = false;
        loop {
            let Some((m, auto)) = self.undo.pop() else {
                return undone_anything;
            };
            revert_move(&mut self.piles, &m);
            undone_anything = true;
            if !auto {
                return true;
            }
        }
    }

    pub fn is_won(&self) -> bool {
        self.rules.is_won(&self.piles)
    }
}

trait StdRngFromSeed {
    fn from_seed_u64(seed: u64) -> Self;
}

impl StdRngFromSeed for rand::rngs::StdRng {
    fn from_seed_u64(seed: u64) -> Self {
        use rand::SeedableRng;
        rand::rngs::StdRng::seed_from_u64(seed)
    }
}
