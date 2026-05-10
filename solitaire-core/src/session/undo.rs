//! Undo stack — last-in-first-out history of applied moves.

use super::moves::Move;

#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    history: Vec<Move>,
}

impl UndoStack {
    pub fn push(&mut self, m: Move) {
        self.history.push(m);
    }

    pub fn pop(&mut self) -> Option<Move> {
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
