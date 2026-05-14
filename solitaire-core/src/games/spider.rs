//! Spider — 10 cascades, 8 foundations, 1 stock, 2 decks (104 cards).
//!
//! Suit count is configurable (1 / 2 / 4); 1-suit Spades is the default
//! beginner variant. Multi-card tableau moves require a SUITED
//! descending tail. Complete K→A suited runs at the top of any cascade
//! auto-collapse to a foundation via `after_move`.

use agg_gui::geometry::Rect;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::cards::{spider_deck, Card, Rank, Suit};
use crate::piles::{PileId, PileKind, PileLayout, PileSet, PileSlot};
use crate::session::Move;

use super::{GameRules, CARD_ASPECT};

const FOUND_FIRST: PileId = 0;
const FOUND_LAST: PileId = 7;
const STOCK: PileId = 8;
const CASCADE_FIRST: PileId = 9;
const CASCADE_LAST: PileId = 18;
const N_CASCADES: usize = 10;

/// Top row has 8 foundations + a 1-column gap + stock = 10 columns.
const TOP_COLS: usize = 10;
/// Vertical budget in card-heights — top row + tableau fan. The normal
/// fan step is intentionally tight, so a typical Spider column still
/// fits without compacting suited runs differently from other cards.
const VERT_BUDGET_CARDS: f64 = 5.0;

pub struct Spider {
    pub suit_count: u8,
    /// Suit used by 1-suit Spider. Ignored when `suit_count > 1`.
    /// Players who want a beginner deal pick this in the Options menu
    /// (matches the "Spider Solitaire 1-suit" variant their wife / kid
    /// plays). Defaults to Spades since that's what the original code
    /// hard-coded.
    pub one_suit: Suit,
}

impl Spider {
    pub const fn new(suit_count: u8) -> Self {
        Self {
            suit_count,
            one_suit: Suit::Spades,
        }
    }
    pub const fn one_suit() -> Self {
        Self::new(1)
    }
    pub const fn one_suit_of(suit: Suit) -> Self {
        Self {
            suit_count: 1,
            one_suit: suit,
        }
    }
    pub const fn two_suit() -> Self {
        Self::new(2)
    }
    pub const fn four_suit() -> Self {
        Self::new(4)
    }

    /// Suits used for this game's deck. Driven by `suit_count`; for
    /// 1-suit Spider the active suit is `self.one_suit`.
    fn suits(&self) -> Vec<Suit> {
        match self.suit_count {
            1 => vec![self.one_suit],
            2 => vec![Suit::Spades, Suit::Hearts],
            _ => vec![Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs],
        }
    }
}

impl Default for Spider {
    fn default() -> Self {
        Self::one_suit()
    }
}

fn is_cascade(id: PileId) -> bool {
    (CASCADE_FIRST..=CASCADE_LAST).contains(&id)
}

fn is_foundation(id: PileId) -> bool {
    (FOUND_FIRST..=FOUND_LAST).contains(&id)
}

/// Suited descending — Spider's multi-card move predicate.
fn is_suited_run(cards: &[Card]) -> bool {
    if cards.iter().any(|c| !c.face_up) {
        return false;
    }
    let suit = cards[0].suit;
    for w in cards.windows(2) {
        if w[0].suit != suit || w[1].suit != suit {
            return false;
        }
        if Some(w[1].rank) != w[0].rank.next_down() {
            return false;
        }
    }
    true
}

/// Top 13 cards of `pile` form a SUITED K-down-to-A run.
fn has_complete_run_on_top(pile: &crate::piles::Pile) -> bool {
    if pile.cards.len() < 13 {
        return false;
    }
    let tail = &pile.cards[pile.cards.len() - 13..];
    if !is_suited_run(tail) {
        return false;
    }
    tail[0].rank == Rank::King && tail[12].rank == Rank::Ace
}

fn suited_run_len_on_top(pile: &crate::piles::Pile) -> usize {
    let Some(top) = pile.cards.last() else {
        return 0;
    };
    if !top.face_up {
        return 0;
    }
    let mut len = 1;
    for pair in pile.cards.windows(2).rev() {
        let lower = &pair[1];
        let higher = &pair[0];
        if !higher.face_up
            || higher.suit != lower.suit
            || Some(lower.rank) != higher.rank.next_down()
        {
            break;
        }
        len += 1;
    }
    len
}

fn destination_run_score(piles: &PileSet, dst: PileId) -> usize {
    let pile = piles.get(dst);
    if pile.is_empty() {
        0
    } else {
        suited_run_len_on_top(pile)
    }
}

impl GameRules for Spider {
    fn pile_layout(&self, rect: Rect) -> Vec<PileSlot> {
        // 10 columns horizontally (max of top-row count and cascade
        // count, both = 10 once the foundation gap is included).
        let col_gap = 10.0;
        let row_gap = 12.0;
        let card_w_by_width = (rect.width - col_gap * (TOP_COLS as f64 - 1.0)) / TOP_COLS as f64;
        let card_h_by_height = (rect.height - row_gap) / VERT_BUDGET_CARDS;
        let card_h = (card_w_by_width * CARD_ASPECT).min(card_h_by_height);
        let card_w = card_h / CARD_ASPECT;
        let col_pitch = card_w + col_gap;
        let used_w = TOP_COLS as f64 * card_w + (TOP_COLS as f64 - 1.0) * col_gap;
        let left = rect.x + (rect.width - used_w) / 2.0;
        let top_row_origin_y = rect.y + rect.height - card_h;
        let tableau_origin_y = top_row_origin_y - (card_h + row_gap);
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
        let mut out = Vec::with_capacity(19);
        // 8 foundations across columns 0..7.
        for i in 0..8u8 {
            out.push(mk(
                FOUND_FIRST + i,
                PileKind::Foundation,
                PileLayout::Stacked,
                i as f64,
                top_row_origin_y,
            ));
        }
        // Stock at column 9 (column 8 left as a visual gap).
        out.push(mk(
            STOCK,
            PileKind::Stock,
            PileLayout::Stacked,
            9.0,
            top_row_origin_y,
        ));
        // 10 cascades on the row below, with uniform visible fan spacing.
        for i in 0..N_CASCADES as u8 {
            out.push(mk(
                CASCADE_FIRST + i,
                PileKind::Tableau,
                PileLayout::FannedDown,
                i as f64,
                tableau_origin_y,
            ));
        }
        out
    }

    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng) {
        let mut deck = spider_deck(&self.suits());
        deck.shuffle(rng);
        let mut iter = deck.into_iter();
        // Cascades 0..3 get 6 cards, cascades 4..9 get 5 cards. Top card
        // face-up.
        for col in 0..N_CASCADES {
            let n = if col < 4 { 6 } else { 5 };
            for j in 0..n {
                let mut card = iter.next().expect("104 card spider deck");
                if j == n - 1 {
                    card.face_up = true;
                }
                piles.get_mut(CASCADE_FIRST + col as u8).cards.push(card);
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
        let take = m.take as usize;
        if from.cards.len() < take {
            return false;
        }
        let moved = &from.cards[from.cards.len() - take..];

        // Stock broadcast: stock → cascade, take=1, flip_moved=true.
        // Generated only by on_pile_click.
        if m.from == STOCK && is_cascade(m.to) {
            return m.take == 1 && m.flip_moved;
        }

        // The complete-run auto-collapse: cascade → foundation, take=13,
        // generated only by `after_move`.
        if is_cascade(m.from) && is_foundation(m.to) {
            return take == 13
                && to.is_empty()
                && is_suited_run(moved)
                && moved[0].rank == Rank::King;
        }

        if moved.iter().any(|c| !c.face_up) {
            return false;
        }

        // Cascade → cascade.
        if is_cascade(m.from) && is_cascade(m.to) {
            // Multi-card moves require a suited descending tail; single-card
            // moves are always fine on the source side.
            if take > 1 && !is_suited_run(moved) {
                return false;
            }
            let head = &moved[0];
            return match to.top() {
                None => true,
                Some(top) => Some(head.rank) == top.rank.next_down(),
            };
        }

        // Spider has no manual moves to foundations or to stock.
        false
    }

    fn auto_complete_step(&self, _piles: &PileSet) -> Option<Move> {
        None
    }

    fn after_move(&self, piles: &PileSet) -> Option<Move> {
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            let cascade = piles.get(cid);
            if !has_complete_run_on_top(cascade) {
                continue;
            }
            // Find first empty foundation.
            for fid in FOUND_FIRST..=FOUND_LAST {
                if piles.get(fid).is_empty() {
                    let mut m = Move::simple(cid, 13, fid);
                    // If lifting the 13-card run exposes a face-down
                    // card, flip it. Without this the cascade stays
                    // visually "stuck" with a face-down top after the
                    // collapse and the player has to guess that the
                    // game is still progressing. (Drag-drop already
                    // does this via finish_drag — the auto-collapse
                    // path needed parity.)
                    let n = cascade.cards.len();
                    if n > 13 && !cascade.cards[n - 14].face_up {
                        m = m.with_flip_source();
                    }
                    return Some(m);
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
        "spider"
    }

    fn on_pile_click(&self, piles: &PileSet, pile: PileId) -> Vec<Move> {
        if pile != STOCK {
            return Vec::new();
        }
        let stock_len = piles.get(STOCK).len();
        if stock_len < N_CASCADES {
            return Vec::new();
        }
        // Standard Spider: stock click is illegal if any cascade is empty.
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            if piles.get(cid).is_empty() {
                return Vec::new();
            }
        }
        let mut out = Vec::with_capacity(N_CASCADES);
        for col in 0..N_CASCADES {
            out.push(Move::simple(STOCK, 1, CASCADE_FIRST + col as u8).with_flip_moved());
        }
        out
    }

    fn single_click_move(&self, piles: &PileSet, pile: PileId, card_idx: usize) -> Option<Move> {
        if !is_cascade(pile) {
            return None;
        }
        let src = piles.get(pile);
        if card_idx >= src.cards.len() || !src.cards[card_idx].face_up {
            return None;
        }
        let take = src.cards.len() - card_idx;
        let mut candidates: Vec<_> = (CASCADE_FIRST..=CASCADE_LAST)
            .filter(|&dst| dst != pile)
            .map(|dst| {
                (
                    destination_run_score(piles, dst),
                    piles.get(dst).origin_x,
                    dst,
                )
            })
            .collect();
        candidates.sort_by(|(a_run, ax, aid), (b_run, bx, bid)| {
            b_run
                .cmp(a_run)
                .then_with(|| ax.total_cmp(bx))
                .then_with(|| aid.cmp(bid))
        });

        for (_, _, dst) in candidates {
            let mut m = Move::simple(pile, take as u8, dst);
            if card_idx > 0 && !src.cards[card_idx - 1].face_up {
                m = m.with_flip_source();
            }
            if self.legal_move(piles, &m) {
                return Some(m);
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "spider_tests.rs"]
mod tests;
