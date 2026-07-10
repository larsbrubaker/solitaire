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
/// Vertical budget in card-heights. Klondike's tableau starts with up
/// to 7 cards in column 6 and grows from there, but face-down rows
/// fan with the smaller offset and players rarely accumulate beyond
/// ~10 card-positions of total vertical extent (top row + tableau).
const VERT_BUDGET_CARDS: f64 = 4.5;

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
        // Two candidate arrangements — the classic top row (7 columns,
        // stock/waste/foundations above the tableau) and a side-column
        // layout (10 columns: stock+waste left, tableau center,
        // foundations 2x2 right) that frees a full card-height for the
        // tableau on wide viewports. Whichever yields the larger card
        // wins.
        let (fit, arrangement) =
            super::pick_board_fit(rect, COLS, SIDE_COLS, 12.0, 12.0, VERT_BUDGET_CARDS);
        // Stretch tableau fan steps into leftover vertical space (width-
        // bound portrait viewports). Worst-case Klondike column: 6
        // face-down cards under a K→A run of 13 face-up cards.
        let fan_scale =
            super::tableau_fan_scale(rect, &fit, arrangement, 12.0, VERT_BUDGET_CARDS, 6, 13);
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
                        .with_fan_scale(fan_scale),
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
                        .with_fan_scale(fan_scale),
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
mod tests {
    use super::*;
    use crate::cards::Suit;
    use crate::session::{apply_move, GameSession};

    #[test]
    fn deal_distributes_28_cards_to_tableau_and_24_to_stock() {
        let s = GameSession::new(Klondike::new(), 7);
        let total_tableau: usize = (TABLEAU_FIRST..=TABLEAU_LAST)
            .map(|id| s.piles.get(id).len())
            .sum();
        assert_eq!(total_tableau, 28);
        assert_eq!(s.piles.get(STOCK).len(), 24);
        assert_eq!(s.piles.get(WASTE).len(), 0);
        // Each tableau column has its top card face-up.
        for id in TABLEAU_FIRST..=TABLEAU_LAST {
            let p = s.piles.get(id);
            assert!(p.top().unwrap().face_up);
        }
        // Force-pop unused-warning suppression when iterating.
        let _ = s.piles.get(STOCK);
    }

    #[test]
    fn ace_to_empty_foundation_is_legal() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
        let m = Move::simple(TABLEAU_FIRST, 1, FOUND_FIRST);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn two_to_empty_foundation_is_illegal() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Two).face_up());
        let m = Move::simple(TABLEAU_FIRST, 1, FOUND_FIRST);
        assert!(!rules.legal_move(&piles, &m));
    }

    #[test]
    fn alternating_descending_run_to_tableau_is_legal() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        // src tableau has [10♣ face-up], dest tableau has [J♥ face-up].
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
        piles
            .get_mut(TABLEAU_FIRST + 1)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Jack).face_up());
        let m = Move::simple(TABLEAU_FIRST, 1, TABLEAU_FIRST + 1);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn same_color_to_tableau_is_illegal() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ten).face_up());
        piles
            .get_mut(TABLEAU_FIRST + 1)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Jack).face_up());
        let m = Move::simple(TABLEAU_FIRST, 1, TABLEAU_FIRST + 1);
        assert!(!rules.legal_move(&piles, &m));
    }

    #[test]
    fn king_to_empty_tableau_is_legal() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(WASTE)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
        let m = Move::simple(WASTE, 1, TABLEAU_FIRST);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn single_click_top_card_prefers_foundation() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
        piles
            .get_mut(TABLEAU_FIRST + 1)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());

        let m = rules
            .single_click_move(&piles, TABLEAU_FIRST, 0)
            .expect("ace can move to foundation");
        assert_eq!(m.from, TABLEAU_FIRST);
        assert_eq!(m.to, FOUND_FIRST);
        assert_eq!(m.take, 1);
    }

    #[test]
    fn single_click_run_moves_to_leftmost_legal_tableau() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        let src = TABLEAU_FIRST + 4;
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Nine).face_up());

        let left_dst = TABLEAU_FIRST + 1;
        let right_dst = TABLEAU_FIRST + 3;
        for dst in [left_dst, right_dst] {
            piles
                .get_mut(dst)
                .cards
                .push(Card::new(Suit::Diamonds, Rank::Jack).face_up());
        }

        let m = rules
            .single_click_move(&piles, src, 0)
            .expect("10-9 run can move onto either jack");
        assert_eq!(m.from, src);
        assert_eq!(m.to, left_dst);
        assert_eq!(m.take, 2);
    }

    #[test]
    fn stock_click_when_nonempty_dispenses_one_to_waste() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ace));
        let moves = rules.on_pile_click(&piles, STOCK);
        assert_eq!(moves.len(), 1);
        let m = moves[0];
        assert!(rules.legal_move(&piles, &m));
        apply_move(&mut piles, &m);
        assert_eq!(piles.get(WASTE).len(), 1);
        assert!(piles.get(WASTE).top().unwrap().face_up);
    }

    #[test]
    fn wide_rect_picks_side_column_layout() {
        let rules = Klondike::with_draw_count(3);
        let rect = Rect::new(0.0, 0.0, 1600.0, 700.0);
        let slots = rules.pile_layout(rect);
        let top = crate::games::fit_cards(rect, 7, 12.0, 12.0, 4.5);
        let side = crate::games::fit_cards(rect, 10, 12.0, 12.0, 3.5);
        assert!(
            side.card_h > top.card_h,
            "side candidate must win on a wide rect"
        );
        let eq = |a: f64, b: f64| (a - b).abs() < 1e-9;
        // Card size comes from the winning 10-column side fit.
        assert!(eq(slots[STOCK as usize].card_h, side.card_h));
        // Left column: stock on top, waste one row below with a
        // downward vertical fan.
        assert!(eq(slots[STOCK as usize].origin_x, side.left));
        assert!(eq(slots[STOCK as usize].origin_y, side.top_row_origin_y));
        let w = &slots[WASTE as usize];
        assert!(eq(w.origin_x, side.left));
        assert!(eq(w.origin_y, side.top_row_origin_y - side.row_pitch));
        assert_eq!(w.fan_top_n, 3);
        assert!(eq(w.fan_dx, 0.0));
        assert!(eq(w.fan_dy, -side.card_h * 0.27));
        // Tableau: columns 1..=7, flush with the playfield top (full
        // height).
        for i in 0..7u8 {
            let t = &slots[(TABLEAU_FIRST + i) as usize];
            assert!(eq(t.origin_x, side.left + (1 + i) as f64 * side.col_pitch));
            assert!(eq(t.origin_y, side.top_row_origin_y));
        }
        // Foundations: 2x2 grid in columns 8 and 9.
        for i in 0..4u8 {
            let f = &slots[(FOUND_FIRST + i) as usize];
            assert!(eq(
                f.origin_x,
                side.left + (8 + i % 2) as f64 * side.col_pitch
            ));
            assert!(eq(
                f.origin_y,
                side.top_row_origin_y - (i / 2) as f64 * side.row_pitch
            ));
        }
    }

    #[test]
    fn tall_rect_keeps_top_row_layout() {
        let rules = Klondike::with_draw_count(3);
        let slots = rules.pile_layout(Rect::new(0.0, 0.0, 390.0, 800.0));
        let eq = |a: f64, b: f64| (a - b).abs() < 1e-6;
        // Pin the historical top-row layout: width-bound cards of
        // (390 - 6*12) / 7 = 45.428… wide, aspect 1.4.
        let card_w = 318.0 / 7.0;
        let card_h = card_w * crate::games::CARD_ASPECT;
        let col_pitch = card_w + 12.0;
        assert!(eq(slots[STOCK as usize].card_h, card_h));
        assert!(eq(slots[STOCK as usize].origin_x, 0.0));
        assert!(eq(slots[STOCK as usize].origin_y, 800.0 - card_h));
        let w = &slots[WASTE as usize];
        assert!(eq(w.origin_x, col_pitch));
        assert!(eq(w.origin_y, 800.0 - card_h));
        assert_eq!(w.fan_top_n, 3);
        assert!(eq(w.fan_dx, card_w * 0.27));
        assert!(eq(w.fan_dy, 0.0));
        // Foundations start in column 3 of the top row; tableau sits
        // one row-pitch below, starting at column 0.
        assert!(eq(slots[FOUND_FIRST as usize].origin_x, 3.0 * col_pitch));
        assert!(eq(slots[FOUND_FIRST as usize].origin_y, 800.0 - card_h));
        assert!(eq(slots[TABLEAU_FIRST as usize].origin_x, 0.0));
        assert!(eq(
            slots[TABLEAU_FIRST as usize].origin_y,
            800.0 - card_h - (card_h + 12.0)
        ));
    }

    #[test]
    fn portrait_rect_scales_tableau_fans() {
        let rules = Klondike::new();
        let rect = Rect::new(0.0, 0.0, 390.0, 800.0);
        let slots = rules.pile_layout(rect);
        let scale = slots[TABLEAU_FIRST as usize].fan_scale;
        assert!(scale > 1.0, "portrait rect must stretch tableau fans");
        assert!(scale <= 2.0);
        for i in 0..COLS as u8 {
            assert_eq!(slots[(TABLEAU_FIRST + i) as usize].fan_scale, scale);
        }
        // Only tableau piles stretch — stock/waste/foundations keep 1.0
        // (and the waste's top-N fan offsets are untouched).
        assert_eq!(slots[STOCK as usize].fan_scale, 1.0);
        assert_eq!(slots[WASTE as usize].fan_scale, 1.0);
        for i in 0..4u8 {
            assert_eq!(slots[(FOUND_FIRST + i) as usize].fan_scale, 1.0);
        }
        // Worst-case column (6 face-down + K→A run of 13 face-up) must
        // still fit above the playfield bottom at this scale.
        let mut pile = crate::piles::Pile::from_slot(&slots[TABLEAU_FIRST as usize]);
        for _ in 0..6 {
            pile.cards
                .push(Card::new(crate::cards::Suit::Spades, Rank::King));
        }
        for _ in 0..13 {
            pile.cards
                .push(Card::new(crate::cards::Suit::Spades, Rank::King).face_up());
        }
        let (_, y_bottom) = pile.position_for(pile.cards.len() - 1);
        assert!(
            y_bottom >= rect.y,
            "worst-case pile bottom {y_bottom} overflows the playfield"
        );
    }

    #[test]
    fn height_bound_rect_keeps_default_fan_scale() {
        let rules = Klondike::new();
        let slots = rules.pile_layout(Rect::new(0.0, 0.0, 1600.0, 700.0));
        for i in 0..COLS as u8 {
            let s = slots[(TABLEAU_FIRST + i) as usize].fan_scale;
            assert!(
                (s - 1.0).abs() < 1e-9,
                "height-bound fit must not stretch fans, got {s}"
            );
        }
    }

    #[test]
    fn stock_click_when_empty_recycles_waste() {
        let rules = Klondike::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        for r in [Rank::Two, Rank::Three, Rank::Four] {
            piles
                .get_mut(WASTE)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        let moves = rules.on_pile_click(&piles, STOCK);
        let m = moves[0];
        assert!(rules.legal_move(&piles, &m));
        apply_move(&mut piles, &m);
        assert_eq!(piles.get(STOCK).len(), 3);
        assert_eq!(piles.get(WASTE).len(), 0);
        // After recycle, all stock cards face-down.
        assert!(piles.get(STOCK).cards.iter().all(|c| !c.face_up));
    }
}
