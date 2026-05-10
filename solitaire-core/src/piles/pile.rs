//! `Pile` — a stack of cards with a kind, a layout, and an origin.

use crate::cards::Card;
use crate::consts::{CARD_H, CARD_W};

use super::layout::PileLayout;
use super::PileId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PileKind {
    Stock,
    Waste,
    Foundation,
    Tableau,
    /// FreeCell free-cell slot — holds at most one card.
    Cell,
}

#[derive(Clone, Debug)]
pub struct Pile {
    pub id: PileId,
    pub kind: PileKind,
    pub layout: PileLayout,
    /// Y-up bottom-left of card[0] in virtual playfield coordinates.
    pub origin_x: f64,
    pub origin_y: f64,
    pub cards: Vec<Card>,
}

impl Pile {
    pub fn new(
        id: PileId,
        kind: PileKind,
        layout: PileLayout,
        origin_x: f64,
        origin_y: f64,
    ) -> Self {
        Self {
            id,
            kind,
            layout,
            origin_x,
            origin_y,
            cards: Vec::new(),
        }
    }

    pub fn top(&self) -> Option<&Card> {
        self.cards.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut Card> {
        self.cards.last_mut()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Y-up position of the bottom-left of card at `idx`.
    pub fn position_for(&self, idx: usize) -> (f64, f64) {
        let mut y = self.origin_y;
        for i in 1..=idx {
            let prev_face_up = self.cards.get(i - 1).map(|c| c.face_up).unwrap_or(false);
            y += self.layout.dy_for(prev_face_up);
        }
        (self.origin_x, y)
    }

    /// Bounding rect of the empty slot (card-shaped placeholder painted
    /// when the pile is empty). Same x and CARD_W/CARD_H as a card would
    /// occupy at index 0.
    pub fn empty_slot_rect(&self) -> (f64, f64, f64, f64) {
        (self.origin_x, self.origin_y, CARD_W, CARD_H)
    }

    /// Y-up bounding rect of the card at `idx`: (x, y, w, h).
    pub fn card_rect(&self, idx: usize) -> (f64, f64, f64, f64) {
        let (x, y) = self.position_for(idx);
        (x, y, CARD_W, CARD_H)
    }
}
