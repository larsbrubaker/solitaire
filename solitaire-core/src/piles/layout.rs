//! Pile layout — how successive cards are positioned relative to the pile origin.

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
}

impl PileLayout {
    /// Y-up offset between card[idx] and card[idx-1] given whether the
    /// previous card was face-up. Returns a NEGATIVE number for FannedDown
    /// (later cards lower on screen = smaller numerical Y).
    pub fn dy_for(self, prev_face_up: bool) -> f64 {
        match self {
            PileLayout::Stacked => 0.0,
            PileLayout::FannedDown => {
                if prev_face_up {
                    -TABLEAU_FAN_DOWN
                } else {
                    -TABLEAU_FAN_DOWN_FACEDOWN
                }
            }
        }
    }

    /// Total Y-up height occupied by a pile of `n` cards under this layout
    /// when laid out with the supplied face-up flags. Useful to clamp
    /// scrolling or to compute hit-test envelopes.
    pub fn pile_height(self, face_ups: &[bool]) -> f64 {
        let n = face_ups.len();
        if n == 0 {
            return 0.0;
        }
        match self {
            PileLayout::Stacked => CARD_H,
            PileLayout::FannedDown => {
                // First card is full height; each subsequent card adds its
                // fan offset (we report unsigned height here regardless of
                // Y-direction).
                let mut h = CARD_H;
                for i in 1..n {
                    let prev_face_up = face_ups[i - 1];
                    h += if prev_face_up {
                        TABLEAU_FAN_DOWN
                    } else {
                        TABLEAU_FAN_DOWN_FACEDOWN
                    };
                }
                h
            }
        }
    }
}
