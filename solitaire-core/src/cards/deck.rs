//! Deck construction + deterministic shuffling.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use super::{Card, Rank, Suit};

/// 52 cards, all face-down, deck_id 0. Order: Spades A..K, Hearts A..K,
/// Diamonds A..K, Clubs A..K.
pub fn standard_deck() -> Vec<Card> {
    let mut out = Vec::with_capacity(52);
    for suit in Suit::ALL {
        for rank in Rank::ALL {
            out.push(Card::new(suit, rank));
        }
    }
    out
}

/// `standard_deck()` shuffled with a deterministic seed. Used by all
/// `GameRules::deal` implementations — pass an `rng` derived from the
/// player-chosen seed (or a wall-clock seed for "random deal").
pub fn shuffled_seeded(seed: u64) -> Vec<Card> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut deck = standard_deck();
    deck.shuffle(&mut rng);
    deck
}

/// Spider deck: always 104 cards, split across the caller's chosen
/// `suits` set. `deck_id` increments per copy so undo can distinguish
/// duplicate (suit, rank) pairs. Caller is responsible for `suits` being
/// 1, 2, or 4 entries (the 104-card total only divides cleanly by those).
pub fn spider_deck(suits: &[Suit]) -> Vec<Card> {
    let copies = 104 / (suits.len() * 13);
    let mut out = Vec::with_capacity(104);
    for d in 0..copies as u8 {
        for &suit in suits {
            for rank in Rank::ALL {
                let mut c = Card::new(suit, rank);
                c.deck_id = d;
                out.push(c);
            }
        }
    }
    out
}
