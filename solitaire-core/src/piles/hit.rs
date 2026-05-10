//! Hit-testing helpers — given a Y-up cursor point, find the topmost card
//! (or empty pile slot) under it.

use super::layout::PileLayout;
use super::pile::{Pile, PileKind};
use super::PileId;

/// Static layout slot used by `GameRules::pile_layout` to declare which
/// piles a variant has and where they live.
#[derive(Clone, Copy, Debug)]
pub struct PileSlot {
    pub id: PileId,
    pub kind: PileKind,
    pub layout: PileLayout,
    pub origin_x: f64,
    pub origin_y: f64,
}

/// What was under the mouse: a specific card index in a pile, the empty
/// slot of an empty pile, or nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitResult {
    Card { pile: PileId, card_idx: usize },
    EmptySlot { pile: PileId },
}

impl Pile {
    /// Hit-test against a Y-up cursor point. Iterates from the topmost
    /// card downward so overlapping fans return the visually-front card.
    /// If the pile is empty and the cursor is over the empty slot, returns
    /// `HitResult::EmptySlot`.
    pub fn hit_test(&self, x: f64, y: f64) -> Option<HitResult> {
        if self.cards.is_empty() {
            let (sx, sy, sw, sh) = self.empty_slot_rect();
            if x >= sx && x <= sx + sw && y >= sy && y <= sy + sh {
                return Some(HitResult::EmptySlot { pile: self.id });
            }
            return None;
        }
        // Top-N-fanned piles (Klondike waste in 3-draw) only let the
        // topmost card be picked up — older fanned cards are decorative.
        if self.fan_top_n > 0 {
            let top = self.cards.len() - 1;
            let (cx, cy, cw, ch) = self.card_rect(top);
            if x >= cx && x <= cx + cw && y >= cy && y <= cy + ch {
                return Some(HitResult::Card {
                    pile: self.id,
                    card_idx: top,
                });
            }
            return None;
        }
        // Iterate from the topmost card backward — the LAST card painted is
        // the FIRST to receive the click (it's drawn on top of earlier ones).
        for idx in (0..self.cards.len()).rev() {
            let (cx, cy, cw, ch) = self.card_rect(idx);
            if x >= cx && x <= cx + cw && y >= cy && y <= cy + ch {
                return Some(HitResult::Card {
                    pile: self.id,
                    card_idx: idx,
                });
            }
        }
        None
    }
}
