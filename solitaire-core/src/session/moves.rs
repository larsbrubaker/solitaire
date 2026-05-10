//! `Move` — one atomic transfer of cards from one pile to another.
//!
//! All four solitaire variants reduce to compositions of this single
//! operation:
//!
//! - Drag-drop: `from = source pile`, `take = N`, `to = destination`.
//! - Klondike stock click: `from = Stock`, `take = 1`, `to = Waste`,
//!   `flip_moved = true` (face-down stock card becomes face-up waste).
//! - Klondike stock recycle: `from = Waste`, `take = waste.len()`,
//!   `to = Stock`, `flip_moved = true`, `reverse_order = true`.
//! - Klondike auto-flip after a tableau move: `flip_source_after = true`
//!   reveals the newly-exposed face-down card on the source tableau.
//! - Spider 13-card run collapse: rules engine emits a normal move with
//!   `take = 13` from a cascade to the next available foundation.

use serde::{Deserialize, Serialize};

use crate::piles::{PileId, PileSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub from: PileId,
    pub take: u8,
    pub to: PileId,
    /// Toggle the `face_up` flag of every moved card.
    pub flip_moved: bool,
    /// After moving, if the new top of the source pile is face-down, flip
    /// it face-up. Used by Klondike's tableau auto-reveal.
    pub flip_source_after: bool,
    /// Reverse the order of moved cards (for stock recycles, where the
    /// top card of waste becomes the BOTTOM of the new stock).
    pub reverse_order: bool,
}

impl Move {
    /// Convenience for the 90% of moves that are simple drag-drops.
    pub fn simple(from: PileId, take: u8, to: PileId) -> Self {
        Self {
            from,
            take,
            to,
            flip_moved: false,
            flip_source_after: false,
            reverse_order: false,
        }
    }

    pub fn with_flip_source(mut self) -> Self {
        self.flip_source_after = true;
        self
    }

    pub fn with_flip_moved(mut self) -> Self {
        self.flip_moved = true;
        self
    }

    pub fn with_reverse(mut self) -> Self {
        self.reverse_order = true;
        self
    }
}

/// Apply `m` to `piles`. Caller is responsible for legality.
pub fn apply_move(piles: &mut PileSet, m: &Move) {
    let take = m.take as usize;
    let from_len = piles.get(m.from).len();
    debug_assert!(take <= from_len, "take {take} > from.len() {from_len}");

    // Pop the top `take` cards from source, preserving their order
    // (cards[from_len-take..from_len] becomes the moved slice in order).
    let mut moved: Vec<_> = piles.get_mut(m.from).cards.split_off(from_len - take);

    if m.flip_moved {
        for c in &mut moved {
            c.flip();
        }
    }
    if m.reverse_order {
        moved.reverse();
    }
    piles.get_mut(m.to).cards.extend(moved);

    if m.flip_source_after {
        if let Some(top) = piles.get_mut(m.from).top_mut() {
            if !top.face_up {
                top.face_up = true;
            }
        }
    }
}

/// Undo `m` against `piles`. Mirror operations of `apply_move` in reverse.
pub fn revert_move(piles: &mut PileSet, m: &Move) {
    // 1. Un-flip the source new top if apply_move auto-flipped it.
    if m.flip_source_after {
        if let Some(top) = piles.get_mut(m.from).top_mut() {
            if top.face_up {
                top.face_up = false;
            }
        }
    }

    let take = m.take as usize;
    let to_len = piles.get(m.to).len();
    debug_assert!(take <= to_len, "revert take {take} > to.len() {to_len}");

    // 2. Take the top `take` cards back off destination.
    let mut moved: Vec<_> = piles.get_mut(m.to).cards.split_off(to_len - take);

    // 3. Reverse the apply transformations in opposite order.
    if m.reverse_order {
        moved.reverse();
    }
    if m.flip_moved {
        for c in &mut moved {
            c.flip();
        }
    }

    piles.get_mut(m.from).cards.extend(moved);
}
