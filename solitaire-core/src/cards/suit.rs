//! Suit + color.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardColor {
    Red,
    Black,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

    pub fn color(self) -> CardColor {
        match self {
            Suit::Hearts | Suit::Diamonds => CardColor::Red,
            Suit::Spades | Suit::Clubs => CardColor::Black,
        }
    }

    /// Unicode glyph for the suit pip — used by the procedural card-face
    /// renderer and HUD displays.
    pub fn glyph(self) -> char {
        match self {
            Suit::Spades => '\u{2660}',
            Suit::Hearts => '\u{2665}',
            Suit::Diamonds => '\u{2666}',
            Suit::Clubs => '\u{2663}',
        }
    }
}
