//! Pile layout — how successive cards are positioned relative to the pile origin.

use crate::cards::Card;

/// Fan-down step for a face-up card, expressed as a fraction of
/// `card_h`. 0.22 matches the historical 28 px against 126 px tall
/// cards, scaling cleanly with whatever card size the game picks.
const FAN_DOWN_FACE_UP: f64 = 0.22;
/// Fan-down step for a face-down card (smaller — nothing readable on
/// the back). 0.11 matches the historical 14 px against 126 px cards.
const FAN_DOWN_FACE_DOWN: f64 = 0.11;

/// How successive cards in a pile are visually offset.
///
/// `pile.origin` is the **bottom-left of card[0]** (the deepest card in the
/// pile, drawn first). Subsequent cards are offset by the layout's per-card
/// vector relative to that origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PileLayout {
    /// All cards stacked exactly on top of card[0]. Only the topmost card
    /// is visible. Used for Stock, Waste, Foundation, and FreeCell cells.
    Stacked,
    /// Cards fan downward in screen terms (smaller numerical Y for later
    /// cards in Y-up). Face-down cards use a smaller offset than face-up
    /// cards because nothing readable is shown on the back.
    FannedDown,
    /// Like `FannedDown`, but consecutive cards that form a SUITED
    /// descending run (same suit, rank stepping down by one) compact
    /// tightly together — a single Spider tableau pile that grows a
    /// long K-down-to-A spades run no longer fills the screen. The
    /// compact step matches the face-down fan offset, so the visual
    /// rhythm reads as "deck-thick" cards stacking onto a partner.
    FannedDownCompactSuited,
}

impl PileLayout {
    /// Y-up offset between card[idx] and card[idx-1] given the
    /// pile's `card_h`. Fan steps scale with card height — so the
    /// fan stays a constant fraction of the card whether the game
    /// pushed cards to 80 px tall or 200 px tall. `prev` and `curr`
    /// can be `None` while a pile is mid-deal (defaults to face-down
    /// step in that case). Returns a NEGATIVE number for any
    /// `FannedDown*` variant.
    pub fn dy_for(self, card_h: f64, prev: Option<&Card>, curr: Option<&Card>) -> f64 {
        match self {
            PileLayout::Stacked => 0.0,
            PileLayout::FannedDown => {
                let prev_face_up = prev.map(|c| c.face_up).unwrap_or(false);
                if prev_face_up {
                    -card_h * FAN_DOWN_FACE_UP
                } else {
                    -card_h * FAN_DOWN_FACE_DOWN
                }
            }
            PileLayout::FannedDownCompactSuited => {
                let Some(p) = prev else {
                    return -card_h * FAN_DOWN_FACE_DOWN;
                };
                if !p.face_up {
                    return -card_h * FAN_DOWN_FACE_DOWN;
                }
                let compact = match curr {
                    Some(c) => c.face_up && p.suit == c.suit && Some(c.rank) == p.rank.next_down(),
                    None => false,
                };
                if compact {
                    -card_h * FAN_DOWN_FACE_DOWN
                } else {
                    -card_h * FAN_DOWN_FACE_UP
                }
            }
        }
    }

    /// Total Y-up height occupied by `cards` under this layout, given
    /// the pile's `card_h`. POSITIVE regardless of fan direction.
    pub fn pile_height(self, card_h: f64, cards: &[Card]) -> f64 {
        let n = cards.len();
        if n == 0 {
            return 0.0;
        }
        match self {
            PileLayout::Stacked => card_h,
            PileLayout::FannedDown | PileLayout::FannedDownCompactSuited => {
                let mut h = card_h;
                for i in 1..n {
                    h += -self.dy_for(card_h, cards.get(i - 1), cards.get(i));
                }
                h
            }
        }
    }
}
