//! Pile layout — how successive cards are positioned relative to the pile origin.

use crate::cards::Card;
use crate::consts::{CARD_H, TABLEAU_FAN_DOWN, TABLEAU_FAN_DOWN_FACEDOWN};

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
    /// Y-up offset between card[idx] and card[idx-1]. `prev` is
    /// card[idx-1], `curr` is card[idx]. Either can be `None` if the
    /// caller is computing the position before the pile is fully
    /// populated (defaults to a face-down step in that case). Returns
    /// a NEGATIVE number for any `FannedDown*` variant (later cards
    /// lower on screen = smaller numerical Y).
    pub fn dy_for(self, prev: Option<&Card>, curr: Option<&Card>) -> f64 {
        match self {
            PileLayout::Stacked => 0.0,
            PileLayout::FannedDown => {
                let prev_face_up = prev.map(|c| c.face_up).unwrap_or(false);
                if prev_face_up {
                    -TABLEAU_FAN_DOWN
                } else {
                    -TABLEAU_FAN_DOWN_FACEDOWN
                }
            }
            PileLayout::FannedDownCompactSuited => {
                let Some(p) = prev else {
                    return -TABLEAU_FAN_DOWN_FACEDOWN;
                };
                if !p.face_up {
                    return -TABLEAU_FAN_DOWN_FACEDOWN;
                }
                // Suited-descending run continuation? Compact tightly.
                let compact = match curr {
                    Some(c) => c.face_up && p.suit == c.suit && Some(c.rank) == p.rank.next_down(),
                    None => false,
                };
                if compact {
                    -TABLEAU_FAN_DOWN_FACEDOWN
                } else {
                    -TABLEAU_FAN_DOWN
                }
            }
        }
    }

    /// Total Y-up height occupied by `cards` under this layout. Used
    /// to clamp scrolling or compute hit-test envelopes. Returns a
    /// POSITIVE number regardless of fan direction.
    pub fn pile_height(self, cards: &[Card]) -> f64 {
        let n = cards.len();
        if n == 0 {
            return 0.0;
        }
        match self {
            PileLayout::Stacked => CARD_H,
            PileLayout::FannedDown | PileLayout::FannedDownCompactSuited => {
                let mut h = CARD_H;
                for i in 1..n {
                    h += -self.dy_for(cards.get(i - 1), cards.get(i));
                }
                h
            }
        }
    }
}
