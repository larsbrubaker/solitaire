//! Rank — Ace..King — with helpers used by `GameRules` impls.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Rank {
    Ace = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
}

impl Rank {
    pub const ALL: [Rank; 13] = [
        Rank::Ace,
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
    ];

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(n: u8) -> Option<Rank> {
        match n {
            1 => Some(Rank::Ace),
            2 => Some(Rank::Two),
            3 => Some(Rank::Three),
            4 => Some(Rank::Four),
            5 => Some(Rank::Five),
            6 => Some(Rank::Six),
            7 => Some(Rank::Seven),
            8 => Some(Rank::Eight),
            9 => Some(Rank::Nine),
            10 => Some(Rank::Ten),
            11 => Some(Rank::Jack),
            12 => Some(Rank::Queen),
            13 => Some(Rank::King),
            _ => None,
        }
    }

    /// One-or-two-character label drawn on the card corner ("A", "2"…"10",
    /// "J", "Q", "K"). Used by the procedural face renderer.
    pub fn label(self) -> &'static str {
        match self {
            Rank::Ace => "A",
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
        }
    }

    /// Returns the rank one above this one (Ace → Two, …, Queen → King).
    /// `King.next_up()` returns `None`.
    pub fn next_up(self) -> Option<Rank> {
        Rank::from_u8(self.as_u8() + 1)
    }

    /// Returns the rank one below this one (King → Queen, …, Two → Ace).
    /// `Ace.next_down()` returns `None`.
    pub fn next_down(self) -> Option<Rank> {
        if self.as_u8() <= 1 {
            None
        } else {
            Rank::from_u8(self.as_u8() - 1)
        }
    }
}
