//! Undo stack — last-in-first-out history of applied moves.
//!
//! Each entry tracks whether the move was player-initiated or generated
//! by a rules-engine `after_move` hook. `pop_until_user` lets `try_undo`
//! unwind any chain of auto follow-ups in a single click.

use super::moves::Move;

#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    history: Vec<(Move, bool)>, // (move, is_auto)
}

impl UndoStack {
    pub fn push_user(&mut self, m: Move) {
        self.history.push((m, false));
    }

    pub fn push_auto(&mut self, m: Move) {
        self.history.push((m, true));
    }

    pub fn pop(&mut self) -> Option<(Move, bool)> {
        self.history.pop()
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}
