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
    /// Logical card width to render at this pile (defaults to
    /// `consts::CARD_W`). Mom's Solitaire shrinks every cell to fit a
    /// 13-column board inside the virtual playfield; other variants
    /// use the standard size.
    pub card_w: f64,
    /// Logical card height — companion to `card_w`.
    pub card_h: f64,
    /// Up to this many of the topmost cards are fanned right by `fan_dx`;
    /// the rest stack at the origin. Used for the Klondike waste pile in
    /// 3-card-draw mode. `0` (default) disables the fan.
    pub fan_top_n: u8,
    /// Per-card horizontal offset within the fan group (Y-up coords).
    pub fan_dx: f64,
    /// When `true`, an Ace at the top of this pile renders as an empty
    /// gap slot rather than as the Ace card. Mom's Solitaire (Montana)
    /// uses this — Aces are gaps, not playable cards. Default `false`.
    pub render_ace_as_gap: bool,
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
            card_w: CARD_W,
            card_h: CARD_H,
            fan_top_n: 0,
            fan_dx: 0.0,
            render_ace_as_gap: false,
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
        let x = self.origin_x + self.fan_x_offset(idx);
        (x, y)
    }

    /// X offset applied to card `idx` due to top-N fan (waste pile in
    /// 3-draw Klondike). Returns 0 when no fan is configured or `idx` is
    /// below the fan group.
    fn fan_x_offset(&self, idx: usize) -> f64 {
        if self.fan_top_n == 0 || self.fan_dx == 0.0 {
            return 0.0;
        }
        let n = self.cards.len();
        let top_n = (self.fan_top_n as usize).min(n);
        let fan_base = n.saturating_sub(top_n);
        if idx >= fan_base {
            (idx - fan_base) as f64 * self.fan_dx
        } else {
            0.0
        }
    }

    /// Bounding rect of the empty slot (card-shaped placeholder painted
    /// when the pile is empty). Uses the pile's per-instance
    /// `card_w` / `card_h`.
    pub fn empty_slot_rect(&self) -> (f64, f64, f64, f64) {
        (self.origin_x, self.origin_y, self.card_w, self.card_h)
    }

    /// Y-up bounding rect of the card at `idx`: (x, y, w, h).
    pub fn card_rect(&self, idx: usize) -> (f64, f64, f64, f64) {
        let (x, y) = self.position_for(idx);
        (x, y, self.card_w, self.card_h)
    }
}
