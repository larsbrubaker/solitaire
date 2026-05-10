//! `Card` — suit + rank + face-up flag + deck id (for multi-deck games).

use serde::{Deserialize, Serialize};

use super::{Rank, Suit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
    pub face_up: bool,
    /// Which deck the card came from. `0` for single-deck games, `0..N` for
    /// Spider variants that use 2+ decks of identical-looking cards.
    pub deck_id: u8,
}

impl Card {
    pub fn new(suit: Suit, rank: Rank) -> Self {
        Self {
            suit,
            rank,
            face_up: false,
            deck_id: 0,
        }
    }

    pub fn face_up(mut self) -> Self {
        self.face_up = true;
        self
    }

    pub fn flip(&mut self) {
        self.face_up = !self.face_up;
    }
}
