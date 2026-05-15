//! Microsoft FreeCell deal generator — reproduces the deal that
//! Jim Horne's original Windows FreeCell would produce for game
//! number `N`. The classic 32,000-deal set ships with #11982 the
//! sole known-unwinnable; Microsoft Solitaire Collection extends
//! the range to 1..1,000,000 with the same algorithm.
//!
//! Algorithm (from Rosetta Code / Shlomi Fish's documentation):
//!
//! 1. Seed the Microsoft C runtime's `rand()` (a 32-bit LCG) with
//!    the game number.
//! 2. Fill an array of 52 cards in fixed order:
//!    Ace of Clubs, Ace of Diamonds, Ace of Hearts, Ace of Spades,
//!    Two of Clubs, …, King of Spades. (Index `p = rank*4 + suit`
//!    with the suit order C/D/H/S.)
//! 3. Shuffle: for `cc` going from 52 down to 1, pick
//!    `nc = rand() % cc`, swap `cards[cc-1]` and `cards[nc]`.
//! 4. Deal row-by-row, 8 cards per row, reading the shuffled array
//!    from index 51 downward. The first 4 columns get 7 cards each
//!    (rows 1..7); the last 4 columns get 6 cards each (rows 1..6).
//!
//! The LCG is the Microsoft C `rand()`:
//!   state = (state * 214013 + 2531011) & 0x7FFFFFFF
//!   rand() = (state >> 16) & 0x7FFF        // 15-bit result
//!
//! `srand(seed)` sets `state = seed`.

use crate::cards::{Card, Rank, Suit};

/// 8 columns of cards, dealt top-to-bottom. Column 0..3 have 7
/// cards, column 4..7 have 6 cards.
pub type FreeCellColumns = [Vec<Card>; 8];

/// Build the deal for Microsoft FreeCell game number `n` (1-indexed
/// to match the in-game numbering). Returns the eight cascades with
/// `cards[0]` at the visual top of each column.
pub fn deal_columns(game_number: u32) -> FreeCellColumns {
    let mut deck = [0u8; 52];
    // Order: Ace of Clubs, Ace of Diamonds, Ace of Hearts, Ace of
    // Spades, Two of Clubs, … King of Spades.
    for (p, slot) in deck.iter_mut().enumerate() {
        let rank = (p / 4) as u8;
        let suit = (p % 4) as u8;
        *slot = rank | (suit << 4);
    }

    let mut state = MsRand::new(game_number);
    let mut cc = 52usize;
    while cc > 0 {
        let nc = (state.next() as usize) % cc;
        cc -= 1;
        deck.swap(cc, nc);
    }

    // Deal row-by-row, reading the shuffled deck from index 51 down.
    let mut columns: FreeCellColumns = Default::default();
    let mut idx = 52usize;
    let mut row = 0usize;
    loop {
        let cards_in_row = if row < 6 { 8 } else { 4 };
        if cards_in_row == 0 {
            break;
        }
        for column in columns.iter_mut().take(cards_in_row) {
            idx -= 1;
            column.push(card_from_byte(deck[idx]));
        }
        row += 1;
        if idx == 0 {
            break;
        }
    }
    columns
}

fn card_from_byte(b: u8) -> Card {
    let rank_idx = b & 0x0F;
    let suit_idx = (b >> 4) & 0x0F;
    Card::new(suit_from_idx(suit_idx), rank_from_idx(rank_idx)).face_up()
}

fn rank_from_idx(i: u8) -> Rank {
    // Ace is index 0 → Rank::Ace, …, King at index 12.
    match i {
        0 => Rank::Ace,
        1 => Rank::Two,
        2 => Rank::Three,
        3 => Rank::Four,
        4 => Rank::Five,
        5 => Rank::Six,
        6 => Rank::Seven,
        7 => Rank::Eight,
        8 => Rank::Nine,
        9 => Rank::Ten,
        10 => Rank::Jack,
        11 => Rank::Queen,
        12 => Rank::King,
        _ => unreachable!("rank index out of range"),
    }
}

fn suit_from_idx(i: u8) -> Suit {
    match i {
        0 => Suit::Clubs,
        1 => Suit::Diamonds,
        2 => Suit::Hearts,
        3 => Suit::Spades,
        _ => unreachable!("suit index out of range"),
    }
}

/// Microsoft C `rand()` — 32-bit LCG with mask 0x7FFFFFFF on state
/// and `(state >> 16) & 0x7FFF` for the 15-bit output. Seeded with
/// the game number via `srand`.
struct MsRand {
    state: u32,
}

impl MsRand {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u32 {
        // Wrapping ops match Microsoft's i32 overflow behavior; the
        // 0x7FFFFFFF mask drops the sign bit.
        self.state = self
            .state
            .wrapping_mul(214013)
            .wrapping_add(2531011)
            & 0x7FFF_FFFF;
        (self.state >> 16) & 0x7FFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Game #1 is the canonical regression deal for Microsoft FreeCell
    /// (e.g. the Wikipedia and Rosetta Code references both quote the
    /// same opening layout). The first row reads:
    ///
    ///   JD 2D 9H JC 5D 7H 7C 5H
    ///
    /// i.e. column 0 top card is J♦, column 1 top is 2♦, … column 7 is 5♥.
    #[test]
    fn game_one_matches_canonical_top_row() {
        let cols = deal_columns(1);
        let top: Vec<(Rank, Suit)> = cols.iter().map(|c| (c[0].rank, c[0].suit)).collect();
        assert_eq!(
            top,
            vec![
                (Rank::Jack, Suit::Diamonds),
                (Rank::Two, Suit::Diamonds),
                (Rank::Nine, Suit::Hearts),
                (Rank::Jack, Suit::Clubs),
                (Rank::Five, Suit::Diamonds),
                (Rank::Seven, Suit::Hearts),
                (Rank::Seven, Suit::Clubs),
                (Rank::Five, Suit::Hearts),
            ]
        );
    }

    #[test]
    fn column_lengths_are_canonical() {
        let cols = deal_columns(1);
        for (c, column) in cols.iter().enumerate().take(4) {
            assert_eq!(column.len(), 7, "column {c} should have 7 cards");
        }
        for (c, column) in cols.iter().enumerate().skip(4) {
            assert_eq!(column.len(), 6, "column {c} should have 6 cards");
        }
    }

    #[test]
    fn deal_uses_all_52_cards_exactly_once() {
        let cols = deal_columns(617);
        let mut seen = std::collections::HashSet::new();
        for col in cols.iter() {
            for c in col {
                assert!(seen.insert((c.suit, c.rank)), "duplicate card {c:?}");
            }
        }
        assert_eq!(seen.len(), 52);
    }

    #[test]
    fn different_games_give_different_layouts() {
        let a = deal_columns(1);
        let b = deal_columns(2);
        assert_ne!(a, b);
    }
}
