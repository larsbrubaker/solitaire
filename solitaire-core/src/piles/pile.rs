//! `Pile` — a stack of cards with a kind, a layout, and an origin.

use crate::cards::Card;

use super::hit::PileSlot;
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
    /// Y-up bottom-left of card[0] in SCREEN coordinates. Updated by
    /// `PileSet::update_layout` whenever the playfield rect changes
    /// (window resize, sidebar→standard chrome flip).
    pub origin_x: f64,
    pub origin_y: f64,
    pub cards: Vec<Card>,
    /// Card width in SCREEN pixels — chosen by the game's
    /// `pile_layout` based on available playfield width and number of
    /// columns.
    pub card_w: f64,
    /// Card height in SCREEN pixels. Variants pick the 5:7 ratio (
    /// `card_h = card_w * 1.4`) to match the standard playing-card
    /// aspect.
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
    /// Initialise from a `PileSlot`. The pile starts empty; cards are
    /// pushed by `GameRules::deal`.
    pub fn from_slot(slot: &PileSlot) -> Self {
        Self {
            id: slot.id,
            kind: slot.kind,
            layout: slot.layout,
            origin_x: slot.origin_x,
            origin_y: slot.origin_y,
            cards: Vec::new(),
            card_w: slot.card_w,
            card_h: slot.card_h,
            fan_top_n: slot.fan_top_n,
            fan_dx: slot.fan_dx,
            render_ace_as_gap: slot.render_ace_as_gap,
        }
    }

    /// Re-apply a slot's layout to an existing pile WITHOUT touching
    /// its card stack. Used by `PileSet::update_layout` when the
    /// playfield rect changes (resize, chrome-mode flip).
    pub fn apply_slot(&mut self, slot: &PileSlot) {
        debug_assert_eq!(self.id, slot.id);
        self.kind = slot.kind;
        self.layout = slot.layout;
        self.origin_x = slot.origin_x;
        self.origin_y = slot.origin_y;
        self.card_w = slot.card_w;
        self.card_h = slot.card_h;
        self.fan_top_n = slot.fan_top_n;
        self.fan_dx = slot.fan_dx;
        self.render_ace_as_gap = slot.render_ace_as_gap;
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
            let prev_prev = if i >= 2 { self.cards.get(i - 2) } else { None };
            let prev = self.cards.get(i - 1);
            let curr = self.cards.get(i);
            y += self.layout.dy_for(self.card_h, prev_prev, prev, curr);
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
