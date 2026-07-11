//! Klondike solitaire — 7 tableau columns, 4 foundations, 1 stock, 1 waste.
//! Draw count is configurable (1 by default, 3 for the Microsoft "Classic"
//! 3-card-draw variant). Both modes share slug `klondike`.

use agg_gui::geometry::Rect;
use rand::rngs::StdRng;

use crate::cards::{Card, Rank};
use crate::piles::{PileId, PileKind, PileLayout, PileSet, PileSlot};
use crate::session::Move;

use super::GameRules;

// Pile ids:
pub(super) const STOCK: PileId = 0;
pub(super) const WASTE: PileId = 1;
pub(super) const FOUND_FIRST: PileId = 2;
pub(super) const FOUND_LAST: PileId = 5;
pub(super) const TABLEAU_FIRST: PileId = 6;
pub(super) const TABLEAU_LAST: PileId = 12;

/// Number of tableau columns (= horizontal layout width budget).
const COLS: usize = 7;
/// Total columns in the side-column arrangement: 1 left (stock +
/// waste) + 7 tableau + 2 right (foundations 2x2).
const SIDE_COLS: usize = 10;
/// Total columns in the stacked side-column arrangement: one left
/// column (stock over waste), 7 tableau columns, and one right column
/// holding all 4 foundations in a single overlapping stack. One fewer
/// column than [`SIDE_COLS`], so cards get wider whenever width binds.
const STACKED_COLS: usize = 9;
/// Vertical budget in card-heights for the top-row layout. Sized for a
/// TYPICAL tableau depth, not the worst case — a pile that grows past
/// the reserved space compresses its fan (`Pile::max_fan_extent`)
/// rather than shrinking every card.
const VERT_BUDGET_CARDS: f64 = 3.6;
/// Vertical budget in card-heights for the side-column layouts (the
/// tableau spans the full playfield height, so it needs fewer reserved
/// card-heights than the top-row layout).
const SIDE_BUDGET_CARDS: f64 = 2.8;
/// FLOOR for the vertical step between successive foundation-slot
/// origins in the stacked side column, as a fraction of card height.
/// `stacked_side_step` spreads the 4 slots wider than this when the
/// column has room; this is the cramped-viewport minimum.
const STACKED_FOUNDATION_STEP: f64 = 0.28;

pub struct Klondike {
    pub draw_count: u8,
}

impl Klondike {
    /// Standard Klondike — 1-card draw.
    pub const fn new() -> Self {
        Self { draw_count: 1 }
    }

    /// Klondike with a configurable draw count (1 = standard, 3 = Microsoft
    /// "Classic"). Both share slug `klondike`; the user picks via menu.
    pub const fn with_draw_count(draw_count: u8) -> Self {
        Self { draw_count }
    }
}

impl Default for Klondike {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn is_tableau(id: PileId) -> bool {
    (TABLEAU_FIRST..=TABLEAU_LAST).contains(&id)
}

fn is_foundation(id: PileId) -> bool {
    (FOUND_FIRST..=FOUND_LAST).contains(&id)
}

/// Are `top` and `cand` an alternating-color, descending pair? `cand` is
/// the card being placed on top of `top`.
pub(super) fn alt_color_descending(top: &Card, cand: &Card) -> bool {
    if top.suit.color() == cand.suit.color() {
        return false;
    }
    Some(cand.rank) == top.rank.next_down()
}

/// Same suit, ascending pair: `cand` is being placed on top of `top` in a
/// foundation pile.
pub(super) fn same_suit_ascending(top: &Card, cand: &Card) -> bool {
    top.suit == cand.suit && Some(cand.rank) == top.rank.next_up()
}

/// Check that `cards` form a valid alternating-color descending run.
/// All cards must be face-up. Used to validate multi-card tableau moves.
pub(super) fn is_valid_run(cards: &[Card]) -> bool {
    if cards.iter().any(|c| !c.face_up) {
        return false;
    }
    for w in cards.windows(2) {
        if !alt_color_descending(&w[0], &w[1]) {
            return false;
        }
    }
    true
}

impl GameRules for Klondike {
    fn pile_layout(&self, rect: Rect) -> Vec<PileSlot> {
        // Three candidate arrangements — the classic top row (7 cols,
        // stock/waste/foundations above the tableau), a side-column
        // layout (10 cols: stock+waste left, tableau center, foundations
        // 2x2 right), and a stacked side-column layout (9 cols: the 4
        // foundations collapse into one overlapping right column, giving
        // the tableau a wider card). Whichever yields the larger card
        // wins; ties prefer TopRow, then SideColumns, then SideStacked.
        let (fit, arrangement, budget) = super::pick_board_fit(
            rect,
            12.0,
            12.0,
            &[
                super::BoardCandidate {
                    arrangement: super::BoardArrangement::TopRow,
                    cols: COLS,
                    vert_budget: VERT_BUDGET_CARDS,
                },
                super::BoardCandidate {
                    arrangement: super::BoardArrangement::SideColumns,
                    cols: SIDE_COLS,
                    vert_budget: SIDE_BUDGET_CARDS,
                },
                super::BoardCandidate {
                    arrangement: super::BoardArrangement::SideStacked,
                    cols: STACKED_COLS,
                    vert_budget: SIDE_BUDGET_CARDS,
                },
            ],
        );
        // Stretch tableau fan steps into leftover vertical space (width-
        // bound portrait viewports); deep piles compress back via
        // `max_fan_extent` below.
        let fan_scale = super::tableau_fan_scale(rect, &fit, arrangement, 12.0, budget);
        // Vertical space the tableau may use — the top row (plus a gap)
        // eats a card-height in TopRow; side layouts span the full
        // playfield. Tableau piles cap their fan extent here.
        let tableau_extent = if arrangement == super::BoardArrangement::TopRow {
            rect.height - fit.card_h - 12.0
        } else {
            rect.height
        };
        let (card_w, card_h) = (fit.card_w, fit.card_h);
        let col_pitch = fit.col_pitch;
        let left = fit.left;
        // Y-up: top row sits near the top of the playfield rect.
        let top_row_origin_y = fit.top_row_origin_y;
        let mk = |id: PileId, kind: PileKind, layout: PileLayout, col: f64, base_y: f64| {
            PileSlot::new(
                id,
                kind,
                layout,
                left + col * col_pitch,
                base_y,
                card_w,
                card_h,
            )
        };
        let mut out = Vec::with_capacity(13);
        match arrangement {
            super::BoardArrangement::TopRow => {
                let tableau_origin_y = top_row_origin_y - fit.row_pitch;
                out.push(mk(
                    STOCK,
                    PileKind::Stock,
                    PileLayout::Stacked,
                    0.0,
                    top_row_origin_y,
                ));
                let mut waste = mk(
                    WASTE,
                    PileKind::Waste,
                    PileLayout::Stacked,
                    1.0,
                    top_row_origin_y,
                );
                if self.draw_count > 1 {
                    // Waste fan width is ~27 % of card width — matches
                    // the historical 24 px against 90 px cards.
                    waste = waste.with_waste_fan(self.draw_count, card_w * 0.27);
                }
                out.push(waste);
                for i in 0..4u8 {
                    out.push(mk(
                        FOUND_FIRST + i,
                        PileKind::Foundation,
                        PileLayout::Stacked,
                        (3 + i) as f64,
                        top_row_origin_y,
                    ));
                }
                for i in 0..COLS as u8 {
                    out.push(
                        mk(
                            TABLEAU_FIRST + i,
                            PileKind::Tableau,
                            PileLayout::FannedDown,
                            i as f64,
                            tableau_origin_y,
                        )
                        .with_fan_scale(fan_scale)
                        .with_max_fan_extent(tableau_extent),
                    );
                }
            }
            super::BoardArrangement::SideColumns => {
                // Left column: stock on top, waste one row below it —
                // the 3-draw fan runs DOWNWARD here (negative Y-up dy)
                // since there is no horizontal room in a single column.
                out.push(mk(
                    STOCK,
                    PileKind::Stock,
                    PileLayout::Stacked,
                    0.0,
                    top_row_origin_y,
                ));
                let mut waste = mk(
                    WASTE,
                    PileKind::Waste,
                    PileLayout::Stacked,
                    0.0,
                    top_row_origin_y - fit.row_pitch,
                );
                if self.draw_count > 1 {
                    waste = waste.with_waste_fan_dy(self.draw_count, -card_h * 0.27);
                }
                out.push(waste);
                // Right two columns: foundations in a 2x2 grid, top
                // row flush with the playfield top.
                for i in 0..4u8 {
                    out.push(mk(
                        FOUND_FIRST + i,
                        PileKind::Foundation,
                        PileLayout::Stacked,
                        (8 + i % 2) as f64,
                        top_row_origin_y - (i / 2) as f64 * fit.row_pitch,
                    ));
                }
                // Tableau spans the full playfield height in the
                // center columns.
                for i in 0..COLS as u8 {
                    out.push(
                        mk(
                            TABLEAU_FIRST + i,
                            PileKind::Tableau,
                            PileLayout::FannedDown,
                            (1 + i) as f64,
                            top_row_origin_y,
                        )
                        .with_fan_scale(fan_scale)
                        .with_max_fan_extent(tableau_extent),
                    );
                }
            }
            super::BoardArrangement::SideStacked => {
                // Left column: stock on top, waste one row below with a
                // downward 3-draw fan (same as the SideColumns arm).
                out.push(mk(
                    STOCK,
                    PileKind::Stock,
                    PileLayout::Stacked,
                    0.0,
                    top_row_origin_y,
                ));
                let mut waste = mk(
                    WASTE,
                    PileKind::Waste,
                    PileLayout::Stacked,
                    0.0,
                    top_row_origin_y - fit.row_pitch,
                );
                if self.draw_count > 1 {
                    waste = waste.with_waste_fan_dy(self.draw_count, -card_h * 0.27);
                }
                out.push(waste);
                // Right column (col 8): all 4 foundations stacked with
                // overlapping origins, stepping downward from the top.
                // The step spreads the 4 slots across the full column
                // height rather than clustering them near the top.
                let found_step = super::stacked_side_step(
                    rect.height,
                    card_h,
                    4,
                    card_h * STACKED_FOUNDATION_STEP,
                );
                for i in 0..4u8 {
                    // Completed foundations stack adjacently from the top;
                    // only the first (lowest-id) slot shows an empty
                    // placeholder so an empty column is a single socket,
                    // not a ladder of stacked empty slots.
                    let mut slot = mk(
                        FOUND_FIRST + i,
                        PileKind::Foundation,
                        PileLayout::Stacked,
                        8.0,
                        top_row_origin_y - i as f64 * found_step,
                    );
                    if i > 0 {
                        slot = slot.with_hidden_empty_slot();
                    }
                    out.push(slot);
                }
                // Tableau: columns 1..=7, full playfield height.
                for i in 0..COLS as u8 {
                    out.push(
                        mk(
                            TABLEAU_FIRST + i,
                            PileKind::Tableau,
                            PileLayout::FannedDown,
                            (1 + i) as f64,
                            top_row_origin_y,
                        )
                        .with_fan_scale(fan_scale)
                        .with_max_fan_extent(tableau_extent),
                    );
                }
            }
        }
        out
    }

    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng) {
        // We use the seed-based deck and ignore `rng` here; sessions seed
        // from the rng's state in `GameSession::new`. To keep the deal
        // truly rng-driven (rather than seed-driven) we shuffle directly:
        use rand::seq::SliceRandom;
        let mut deck = crate::cards::standard_deck();
        deck.shuffle(rng);

        let mut iter = deck.into_iter();
        for col in 0..7u8 {
            for row in 0..=col {
                let mut card = iter.next().expect("52 card deck");
                if row == col {
                    card.face_up = true;
                }
                piles.get_mut(TABLEAU_FIRST + col).cards.push(card);
            }
        }
        for card in iter {
            piles.get_mut(STOCK).cards.push(card);
        }
    }

    fn legal_move(&self, piles: &PileSet, m: &Move) -> bool {
        if m.take == 0 {
            return false;
        }
        let from = piles.get(m.from);
        let to = piles.get(m.to);
        if from.cards.len() < m.take as usize {
            return false;
        }
        let take = m.take as usize;
        let moved = &from.cards[from.cards.len() - take..];

        // ── Stock click: stock → waste, 1..draw_count cards, flip_moved ─
        if m.from == STOCK && m.to == WASTE {
            return m.take >= 1 && m.take <= self.draw_count && m.flip_moved;
        }
        // ── Stock recycle: waste → stock, take=all, reverse, flip_moved ─
        if m.from == WASTE && m.to == STOCK {
            return from.cards.len() == take && m.reverse_order && m.flip_moved;
        }
        // ── Drag/drop moves: all moved cards must be face-up. ───────────
        if moved.iter().any(|c| !c.face_up) {
            return false;
        }

        // Foundation destination:
        if is_foundation(m.to) {
            if take != 1 {
                return false;
            }
            let cand = &moved[0];
            return match to.top() {
                None => cand.rank == Rank::Ace,
                Some(top) => same_suit_ascending(top, cand),
            };
        }

        // Tableau destination:
        if is_tableau(m.to) {
            if !is_valid_run(moved) {
                return false;
            }
            let head = &moved[0];
            return match to.top() {
                None => head.rank == Rank::King,
                Some(top) => alt_color_descending(top, head),
            };
        }

        // Stock / Waste are not valid drop targets.
        false
    }

    fn auto_complete_step(&self, piles: &PileSet) -> Option<Move> {
        // Auto-complete eligibility: every tableau card is face-up.
        for id in TABLEAU_FIRST..=TABLEAU_LAST {
            if piles.get(id).cards.iter().any(|c| !c.face_up) {
                return None;
            }
        }
        // For each non-foundation pile with a top card, see if it fits a
        // foundation. Iterate in priority order: tableaus first, waste
        // last (stock should be empty by this point).
        let mut sources: Vec<PileId> = (TABLEAU_FIRST..=TABLEAU_LAST).collect();
        sources.push(WASTE);
        for src in sources {
            let pile = piles.get(src);
            let Some(top) = pile.top() else { continue };
            if !top.face_up {
                continue;
            }
            for fid in FOUND_FIRST..=FOUND_LAST {
                let f = piles.get(fid);
                let ok = match f.top() {
                    None => top.rank == Rank::Ace,
                    Some(ftop) => same_suit_ascending(ftop, top),
                };
                if ok {
                    return Some(Move::simple(src, 1, fid));
                }
            }
        }
        None
    }

    fn is_won(&self, piles: &PileSet) -> bool {
        for fid in FOUND_FIRST..=FOUND_LAST {
            let f = piles.get(fid);
            if f.cards.len() != 13 {
                return false;
            }
        }
        true
    }

    fn game_slug(&self) -> &'static str {
        "klondike"
    }

    fn on_pile_click(&self, piles: &PileSet, pile: PileId) -> Vec<Move> {
        if pile == STOCK {
            if !piles.get(STOCK).is_empty() {
                let n = (piles.get(STOCK).len() as u8).min(self.draw_count);
                return vec![Move::simple(STOCK, n, WASTE).with_flip_moved()];
            }
            let waste_len = piles.get(WASTE).len();
            if waste_len > 0 {
                return vec![Move {
                    from: WASTE,
                    take: waste_len as u8,
                    to: STOCK,
                    flip_moved: true,
                    flip_source_after: false,
                    reverse_order: true,
                    swap_with_top: false,
                }];
            }
        }
        Vec::new()
    }

    fn single_click_move(&self, piles: &PileSet, pile: PileId, card_idx: usize) -> Option<Move> {
        let src = piles.get(pile);
        if card_idx >= src.cards.len() || !src.cards[card_idx].face_up {
            return None;
        }

        let take = src.cards.len() - card_idx;
        let can_flip_source = is_tableau(pile) && card_idx > 0 && !src.cards[card_idx - 1].face_up;

        // Prefer foundation moves for top cards, matching the double-click
        // shortcut and the usual "send it home" click expectation.
        if take == 1 {
            for dst in FOUND_FIRST..=FOUND_LAST {
                if dst == pile {
                    continue;
                }
                let mut m = Move::simple(pile, 1, dst);
                if can_flip_source {
                    m = m.with_flip_source();
                }
                if self.legal_move(piles, &m) {
                    return Some(m);
                }
            }
        }

        let mut candidates: Vec<_> = (TABLEAU_FIRST..=TABLEAU_LAST)
            .filter(|&dst| dst != pile)
            .map(|dst| (piles.get(dst).origin_x, dst))
            .collect();
        candidates.sort_by(|(ax, aid), (bx, bid)| ax.total_cmp(bx).then_with(|| aid.cmp(bid)));

        for (_, dst) in candidates {
            let mut m = Move::simple(pile, take as u8, dst);
            if can_flip_source {
                m = m.with_flip_source();
            }
            if self.legal_move(piles, &m) {
                return Some(m);
            }
        }
        None
    }
}

// Re-exported so callers don't need to know about the constants.
pub fn is_tableau_pile(id: PileId) -> bool {
    is_tableau(id)
}
pub fn is_foundation_pile(id: PileId) -> bool {
    is_foundation(id)
}
pub const KLONDIKE_STOCK: PileId = STOCK;
pub const KLONDIKE_WASTE: PileId = WASTE;

// `best_klondike_hint` lives in the sibling `klondike_hint` module
// so this file stays under the 800-line cap.

#[cfg(test)]
#[path = "klondike_tests.rs"]
mod tests;
