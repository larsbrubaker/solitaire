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
    /// Up to this many of the topmost cards are fanned by (`fan_dx`,
    /// `fan_dy`) per card; the rest stack at the origin. Used for the
    /// Klondike waste pile in 3-card-draw mode. `0` (default) disables
    /// the fan.
    pub fan_top_n: u8,
    /// Per-card horizontal offset within the fan group (Y-up coords).
    pub fan_dx: f64,
    /// Per-card vertical offset within the fan group (Y-up coords —
    /// negative fans downward on screen). Used when the waste sits in
    /// a side column in the wide-viewport layout.
    pub fan_dy: f64,
    /// When `true`, an Ace at the top of this pile renders as an empty
    /// gap slot rather than as the Ace card. Mom's Solitaire (Montana)
    /// uses this — Aces are gaps, not playable cards. Default `false`.
    pub render_ace_as_gap: bool,
    /// Multiplier applied to the layout's fan steps (`PileLayout::
    /// dy_for`) in `position_for`, so every consumer — painting,
    /// hit-testing, animations — sees the same stretched fan. Card
    /// size and the top-N waste fan are unaffected. Default `1.0`.
    pub fan_scale: f64,
    /// Maximum vertical extent (SCREEN px) the pile's full fan may
    /// occupy. `0.0` (default) = unlimited. When a pile's natural fan
    /// (card height + scaled fan steps) would exceed this, every step
    /// is compressed uniformly so the full extent lands exactly on
    /// `max_fan_extent`. See [`Pile::position_for`] and
    /// [`crate::piles::PileSlot::max_fan_extent`].
    pub max_fan_extent: f64,
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
            fan_dy: slot.fan_dy,
            render_ace_as_gap: slot.render_ace_as_gap,
            fan_scale: slot.fan_scale,
            max_fan_extent: slot.max_fan_extent,
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
        self.fan_dy = slot.fan_dy;
        self.render_ace_as_gap = slot.render_ace_as_gap;
        self.fan_scale = slot.fan_scale;
        self.max_fan_extent = slot.max_fan_extent;
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
        let compression = self.fan_compression();
        let mut y = self.origin_y;
        for i in 1..=idx {
            let prev_prev = if i >= 2 { self.cards.get(i - 2) } else { None };
            let prev = self.cards.get(i - 1);
            let curr = self.cards.get(i);
            // `fan_scale` stretches every fan step and `compression`
            // (<= 1.0) shrinks it back so a deep pile never overruns
            // `max_fan_extent`. This is the single seam all consumers
            // (paint, hit-test, animation) resolve positions through,
            // so they stay consistent.
            y += self.layout.dy_for(self.card_h, prev_prev, prev, curr)
                * self.fan_scale
                * compression;
        }
        let (fan_dx, fan_dy) = self.fan_offset(idx);
        (self.origin_x + fan_dx, y + fan_dy)
    }

    /// Uniform multiplier applied to every (already `fan_scale`-stretched)
    /// fan step so the pile's full vertical extent — card height plus the
    /// summed fan steps — never exceeds `max_fan_extent`. Returns:
    /// - `1.0` when `max_fan_extent` is unlimited (`<= 0.0`), the natural
    ///   fan already fits, or any input is non-finite (no compression);
    /// - `0.0` when `max_fan_extent <= card_h` (collapse to a stack — no
    ///   room for any fan);
    /// - otherwise `(max_fan_extent - card_h) / steps_total`, landing the
    ///   full extent exactly on `max_fan_extent`.
    ///
    /// Guards mirror `games::tableau_fan_scale`: NO NaN may reach a
    /// consumer's clamp, so every division and comparison is checked.
    fn fan_compression(&self) -> f64 {
        if self.max_fan_extent <= 0.0 {
            return 1.0;
        }
        // Natural total fan-step sum (already includes `fan_scale`).
        // `pile_height` returns card_h + summed scaled steps; subtract
        // the card so only the fan steps remain.
        let steps_total = self
            .layout
            .pile_height(self.card_h, self.fan_scale, &self.cards)
            - self.card_h;
        if !steps_total.is_finite() || steps_total <= 0.0 {
            return 1.0;
        }
        if self.card_h + steps_total <= self.max_fan_extent {
            return 1.0;
        }
        if self.max_fan_extent <= self.card_h {
            return 0.0;
        }
        let factor = (self.max_fan_extent - self.card_h) / steps_total;
        if !factor.is_finite() {
            return 1.0;
        }
        factor
    }

    /// (x, y) offset applied to card `idx` due to top-N fan (waste
    /// pile in 3-draw Klondike). Returns (0, 0) when no fan is
    /// configured or `idx` is below the fan group.
    fn fan_offset(&self, idx: usize) -> (f64, f64) {
        if self.fan_top_n == 0 || (self.fan_dx == 0.0 && self.fan_dy == 0.0) {
            return (0.0, 0.0);
        }
        let n = self.cards.len();
        let top_n = (self.fan_top_n as usize).min(n);
        let fan_base = n.saturating_sub(top_n);
        if idx >= fan_base {
            let k = (idx - fan_base) as f64;
            (k * self.fan_dx, k * self.fan_dy)
        } else {
            (0.0, 0.0)
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
