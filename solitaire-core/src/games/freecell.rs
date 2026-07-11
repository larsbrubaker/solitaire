//! FreeCell — 8 cascades, 4 free cells, 4 foundations. No stock.
//!
//! Layout:
//!   Top row:  [Cell0][Cell1][Cell2][Cell3] [gap] [F0][F1][F2][F3]
//!   Below:    [C0][C1][C2][C3][C4][C5][C6][C7]
//!
//! Pile ids:  cells 0..3, foundations 4..7, cascades 8..15.

use agg_gui::geometry::Rect;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::cards::{Card, Rank};
use crate::piles::{PileId, PileKind, PileLayout, PileSet, PileSlot};
use crate::session::Move;

use super::GameRules;

const CELL_FIRST: PileId = 0;
const CELL_LAST: PileId = 3;
const FOUND_FIRST: PileId = 4;
const FOUND_LAST: PileId = 7;
const CASCADE_FIRST: PileId = 8;
const CASCADE_LAST: PileId = 15;

const N_CASCADES: usize = 8;
/// Total columns in the side-column arrangement: 2 left (free cells
/// 2x2) + 8 cascades + 2 right (foundations 2x2).
const SIDE_COLS: usize = 12;
/// Total columns in the stacked side-column arrangement: 1 left (4
/// cells in ONE overlapping column) + 8 cascades + 1 right (4
/// foundations in ONE overlapping column). Two fewer columns than
/// [`SIDE_COLS`], so cards get wider whenever width binds.
const STACKED_COLS: usize = 10;
/// Vertical budget in card-heights for the top-row layout — sized for a
/// typical cascade depth. Deeper cascades compress their fan
/// (`Pile::max_fan_extent`) rather than shrinking every card.
const VERT_BUDGET_CARDS: f64 = 4.0;
/// Vertical budget in card-heights for the side-column layouts (the
/// cascades span the full playfield height).
const SIDE_BUDGET_CARDS: f64 = 3.2;
/// FLOOR for the stacked side-column step (cells & foundations), as a
/// fraction of card height. `stacked_side_step` spreads the 4 slots
/// wider than this when the column has room; this is the cramped-
/// viewport minimum.
const STACKED_STEP: f64 = 0.28;

pub struct FreeCell {
    /// When `Some(n)`, `deal()` reproduces Microsoft FreeCell game
    /// #n via `super::ms_freecell::deal_columns(n)` and ignores the
    /// session's RNG. Microsoft's algorithm validates 31,999 of the
    /// original 32,000 deals as winnable — exactly #11982 is the
    /// known-unwinnable hold-out and is filtered out at the picker.
    pub ms_game_number: Option<u32>,
}

impl FreeCell {
    pub const fn new() -> Self {
        Self {
            ms_game_number: None,
        }
    }

    pub const fn with_ms_game_number(n: u32) -> Self {
        Self {
            ms_game_number: Some(n),
        }
    }
}

impl Default for FreeCell {
    fn default() -> Self {
        Self::new()
    }
}

fn is_cell(id: PileId) -> bool {
    (CELL_FIRST..=CELL_LAST).contains(&id)
}

fn is_foundation(id: PileId) -> bool {
    (FOUND_FIRST..=FOUND_LAST).contains(&id)
}

fn is_cascade(id: PileId) -> bool {
    (CASCADE_FIRST..=CASCADE_LAST).contains(&id)
}

fn alt_color_descending(top: &Card, cand: &Card) -> bool {
    top.suit.color() != cand.suit.color() && Some(cand.rank) == top.rank.next_down()
}

fn same_suit_ascending(top: &Card, cand: &Card) -> bool {
    top.suit == cand.suit && Some(cand.rank) == top.rank.next_up()
}

fn is_valid_run(cards: &[Card]) -> bool {
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

/// Maximum tail length the player can move in one drag — given the
/// number of empty free cells and empty cascades elsewhere on the
/// board. Standard formula: `(1 + free_cells) * 2^empty_cascades`.
/// `dest` is excluded from the empty-cascade count when the
/// destination is itself an empty cascade (you can't use a column as
/// "free space" while you're filling it).
fn max_movable(piles: &PileSet, dest: PileId) -> usize {
    let mut empty_cells: u32 = 0;
    for id in CELL_FIRST..=CELL_LAST {
        if piles.get(id).is_empty() {
            empty_cells += 1;
        }
    }
    let mut empty_cascades: u32 = 0;
    for id in CASCADE_FIRST..=CASCADE_LAST {
        if id == dest {
            continue;
        }
        if piles.get(id).is_empty() {
            empty_cascades += 1;
        }
    }
    ((1 + empty_cells) as usize) << empty_cascades
}

impl GameRules for FreeCell {
    fn pile_layout(&self, rect: Rect) -> Vec<PileSlot> {
        // Three candidate arrangements — the classic top row (8 cols,
        // cells + foundations above the cascades), a side-column layout
        // (12 cols: cells 2x2 left, cascades center, foundations 2x2
        // right), and a stacked side-column layout (10 cols: cells and
        // foundations each collapse into one overlapping column, giving
        // the cascades wider cards). Whichever yields the larger card
        // wins; ties prefer TopRow, then SideColumns, then SideStacked.
        let (fit, arrangement, budget) = super::pick_board_fit(
            rect,
            10.0,
            12.0,
            &[
                super::BoardCandidate {
                    arrangement: super::BoardArrangement::TopRow,
                    cols: N_CASCADES,
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
        // Stretch cascade fan steps into leftover vertical space (width-
        // bound portrait viewports); deep cascades compress back via
        // `max_fan_extent` below.
        let fan_scale = super::tableau_fan_scale(rect, &fit, arrangement, 12.0, budget);
        // Vertical space the cascades may use — TopRow reserves a
        // card-height (plus a gap) for the top row; side layouts span
        // the full playfield.
        let tableau_extent = if arrangement == super::BoardArrangement::TopRow {
            rect.height - fit.card_h - 12.0
        } else {
            rect.height
        };
        let (card_w, card_h) = (fit.card_w, fit.card_h);
        let col_pitch = fit.col_pitch;
        let left = fit.left;
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
        let mut out = Vec::with_capacity(16);
        match arrangement {
            super::BoardArrangement::TopRow => {
                let tableau_origin_y = top_row_origin_y - fit.row_pitch;
                for i in 0..4u8 {
                    out.push(mk(
                        CELL_FIRST + i,
                        PileKind::Cell,
                        PileLayout::Stacked,
                        i as f64,
                        top_row_origin_y,
                    ));
                }
                for i in 0..4u8 {
                    out.push(mk(
                        FOUND_FIRST + i,
                        PileKind::Foundation,
                        PileLayout::Stacked,
                        (4 + i) as f64,
                        top_row_origin_y,
                    ));
                }
                for i in 0..N_CASCADES as u8 {
                    out.push(
                        mk(
                            CASCADE_FIRST + i,
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
                // Left two columns: free cells in a 2x2 grid; right
                // two columns: foundations in a 2x2 grid. Both grids'
                // top rows sit flush with the playfield top; the
                // cascades span the full height in between.
                for i in 0..4u8 {
                    out.push(mk(
                        CELL_FIRST + i,
                        PileKind::Cell,
                        PileLayout::Stacked,
                        (i % 2) as f64,
                        top_row_origin_y - (i / 2) as f64 * fit.row_pitch,
                    ));
                }
                for i in 0..4u8 {
                    out.push(mk(
                        FOUND_FIRST + i,
                        PileKind::Foundation,
                        PileLayout::Stacked,
                        (10 + i % 2) as f64,
                        top_row_origin_y - (i / 2) as f64 * fit.row_pitch,
                    ));
                }
                for i in 0..N_CASCADES as u8 {
                    out.push(
                        mk(
                            CASCADE_FIRST + i,
                            PileKind::Tableau,
                            PileLayout::FannedDown,
                            (2 + i) as f64,
                            top_row_origin_y,
                        )
                        .with_fan_scale(fan_scale)
                        .with_max_fan_extent(tableau_extent),
                    );
                }
            }
            super::BoardArrangement::SideStacked => {
                // Both the cell column (col 0) and foundation column
                // (col 9) stack 4 slots with overlapping origins. The
                // step spreads the 4 slots across the full column height
                // instead of clustering them at the top.
                let stack_step =
                    super::stacked_side_step(rect.height, card_h, 4, card_h * STACKED_STEP, 10.0);
                // Left column (col 0): 4 free cells stacked with
                // overlapping origins, stepping downward from the top.
                for i in 0..4u8 {
                    out.push(mk(
                        CELL_FIRST + i,
                        PileKind::Cell,
                        PileLayout::Stacked,
                        0.0,
                        top_row_origin_y - i as f64 * stack_step,
                    ));
                }
                // Right column (col 9): 4 foundations stacked the same
                // way.
                for i in 0..4u8 {
                    out.push(mk(
                        FOUND_FIRST + i,
                        PileKind::Foundation,
                        PileLayout::Stacked,
                        9.0,
                        top_row_origin_y - i as f64 * stack_step,
                    ));
                }
                // Cascades: columns 1..=8, full playfield height.
                for i in 0..N_CASCADES as u8 {
                    out.push(
                        mk(
                            CASCADE_FIRST + i,
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
        // Microsoft-number path: skip the regular RNG entirely and
        // reproduce Jim Horne's classic deal so the user can replay
        // a familiar Game #N from the original Windows FreeCell set.
        if let Some(game) = self.ms_game_number {
            let cols = super::ms_freecell::deal_columns(game);
            for (col_idx, col) in cols.iter().enumerate() {
                for &card in col {
                    piles
                        .get_mut(CASCADE_FIRST + col_idx as u8)
                        .cards
                        .push(card);
                }
            }
            return;
        }
        let mut deck = crate::cards::standard_deck();
        deck.shuffle(rng);
        // First 4 cascades: 7 cards. Last 4 cascades: 6 cards. All face-up.
        let mut iter = deck.into_iter();
        for col in 0..N_CASCADES {
            let n = if col < 4 { 7 } else { 6 };
            for _ in 0..n {
                let mut card = iter.next().expect("52 card deck");
                card.face_up = true;
                piles.get_mut(CASCADE_FIRST + col as u8).cards.push(card);
            }
        }
        debug_assert!(iter.next().is_none(), "all 52 cards dealt");
    }

    fn legal_move(&self, piles: &PileSet, m: &Move) -> bool {
        if m.take == 0 {
            return false;
        }
        let from = piles.get(m.from);
        let to = piles.get(m.to);
        let take = m.take as usize;
        if from.cards.len() < take {
            return false;
        }
        let moved = &from.cards[from.cards.len() - take..];
        if moved.iter().any(|c| !c.face_up) {
            return false;
        }

        // Foundation destination — single card, same suit ascending,
        // Ace onto empty.
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

        // Free cell destination — single card, cell must be empty.
        if is_cell(m.to) {
            return take == 1 && to.is_empty();
        }

        // Cascade destination.
        if is_cascade(m.to) {
            if !is_valid_run(moved) {
                return false;
            }
            let max_len = max_movable(piles, m.to);
            if take > max_len {
                return false;
            }
            let head = &moved[0];
            return match to.top() {
                None => true, // any card head onto empty cascade.
                Some(top) => alt_color_descending(top, head),
            };
        }

        false
    }

    fn auto_complete_step(&self, piles: &PileSet) -> Option<Move> {
        // No face-down cards in FreeCell, so once every card on the board
        // ranks safely (roughly: every card is at most 2 ranks above the
        // smallest foundation top), auto-complete moves cards up. For a
        // first cut we only auto-flush when SOMETHING is one rank above
        // every foundation's top, which is conservative but safe.
        for src in (CASCADE_FIRST..=CASCADE_LAST).chain(CELL_FIRST..=CELL_LAST) {
            let p = piles.get(src);
            let Some(top) = p.top() else { continue };
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
            if piles.get(fid).cards.len() != 13 {
                return false;
            }
        }
        true
    }

    fn game_slug(&self) -> &'static str {
        "freecell"
    }

    fn single_click_move(&self, piles: &PileSet, pile: PileId, card_idx: usize) -> Option<Move> {
        let src = piles.get(pile);
        if card_idx >= src.cards.len() || !src.cards[card_idx].face_up {
            return None;
        }
        let take = src.cards.len() - card_idx;

        // Single cards prefer foundation, then a useful cascade move, then
        // the leftmost empty free cell. Multi-card runs only move to cascades.
        if take == 1 {
            for dst in FOUND_FIRST..=FOUND_LAST {
                if dst == pile {
                    continue;
                }
                let m = Move::simple(pile, 1, dst);
                if self.legal_move(piles, &m) {
                    return Some(m);
                }
            }
        }

        let mut cascades: Vec<_> = (CASCADE_FIRST..=CASCADE_LAST)
            .filter(|&dst| dst != pile)
            .map(|dst| (piles.get(dst).origin_x, dst))
            .collect();
        cascades.sort_by(|(ax, aid), (bx, bid)| ax.total_cmp(bx).then_with(|| aid.cmp(bid)));
        for (_, dst) in cascades {
            let m = Move::simple(pile, take as u8, dst);
            if self.legal_move(piles, &m) {
                return Some(m);
            }
        }

        if take == 1 {
            for dst in CELL_FIRST..=CELL_LAST {
                if dst == pile {
                    continue;
                }
                let m = Move::simple(pile, 1, dst);
                if self.legal_move(piles, &m) {
                    return Some(m);
                }
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "freecell_tests.rs"]
mod tests;
