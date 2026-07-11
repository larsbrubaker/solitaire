//! Hit-testing helpers — given a Y-up cursor point, find the topmost card
//! (or empty pile slot) under it.

use super::layout::PileLayout;
use super::pile::{Pile, PileKind};
use super::PileId;

/// Layout slot used by `GameRules::pile_layout` to describe one pile's
/// position, size, and rendering config. Origin/size are in SCREEN
/// coordinates (Y-up, viewport-relative); a variant computes them on
/// demand for the playfield rect that `GameWidget` hands it, so cards
/// scale to fill whatever window the user gave us.
#[derive(Clone, Copy, Debug)]
pub struct PileSlot {
    pub id: PileId,
    pub kind: PileKind,
    pub layout: PileLayout,
    pub origin_x: f64,
    pub origin_y: f64,
    pub card_w: f64,
    pub card_h: f64,
    /// Up to this many of the topmost cards fan by (`fan_dx`,
    /// `fan_dy`) per card (Klondike 3-draw waste). `0` means no fan.
    pub fan_top_n: u8,
    /// Per-card X offset within the fan group (positive = rightward).
    pub fan_dx: f64,
    /// Per-card Y-up offset within the fan group (negative = downward
    /// on screen). Used when the waste sits in a side column and fans
    /// toward the bottom instead of to the right.
    pub fan_dy: f64,
    /// `true` for Mom's tableau cells — an Ace top-card paints as a
    /// "gap" placeholder rather than as a face-up Ace.
    pub render_ace_as_gap: bool,
    /// Multiplier applied to the layout's fan steps (`PileLayout::
    /// dy_for`). Games set this above `1.0` on tableau piles when the
    /// winning board arrangement leaves unused vertical space, so fans
    /// spread out for readability. Card size is unaffected; the top-N
    /// waste fan (`fan_dx`/`fan_dy`) is unaffected. Default `1.0`.
    pub fan_scale: f64,
    /// Maximum vertical extent (in SCREEN pixels) the pile's full fan
    /// may occupy — from the top edge of card[0] down to the bottom of
    /// the deepest fanned card. `0.0` (the default) means unlimited: no
    /// compression. Games set this on tableau piles to the vertical
    /// space the winning board layout reserves for the tableau, so a
    /// pile that grows past that budget compresses its fan steps (see
    /// `Pile::position_for`) rather than overflowing the playfield.
    /// Card size and the top-N waste fan are unaffected.
    pub max_fan_extent: f64,
}

impl PileSlot {
    /// Minimal-config slot for the common "stacked / fanned-down,
    /// default card size, no waste-fan, paint Aces normally" case.
    /// Callers customise via the field setters below.
    pub fn new(
        id: PileId,
        kind: PileKind,
        layout: PileLayout,
        origin_x: f64,
        origin_y: f64,
        card_w: f64,
        card_h: f64,
    ) -> Self {
        Self {
            id,
            kind,
            layout,
            origin_x,
            origin_y,
            card_w,
            card_h,
            fan_top_n: 0,
            fan_dx: 0.0,
            fan_dy: 0.0,
            render_ace_as_gap: false,
            fan_scale: 1.0,
            max_fan_extent: 0.0,
        }
    }

    pub fn with_waste_fan(mut self, top_n: u8, dx: f64) -> Self {
        self.fan_top_n = top_n;
        self.fan_dx = dx;
        self
    }

    /// Vertical waste fan — same top-N mechanism as
    /// [`Self::with_waste_fan`] but offsetting each fanned card by
    /// `dy` in Y-up coordinates (pass a NEGATIVE value to fan
    /// downward on screen).
    pub fn with_waste_fan_dy(mut self, top_n: u8, dy: f64) -> Self {
        self.fan_top_n = top_n;
        self.fan_dy = dy;
        self
    }

    pub fn with_ace_as_gap(mut self) -> Self {
        self.render_ace_as_gap = true;
        self
    }

    /// Stretch the layout's fan steps by `scale` (`1.0` = default
    /// spacing). See [`PileSlot::fan_scale`].
    pub fn with_fan_scale(mut self, scale: f64) -> Self {
        self.fan_scale = scale;
        self
    }

    /// Cap the pile's full fan extent at `extent` SCREEN pixels
    /// (`0.0` = unlimited). See [`PileSlot::max_fan_extent`].
    pub fn with_max_fan_extent(mut self, extent: f64) -> Self {
        self.max_fan_extent = extent;
        self
    }
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
