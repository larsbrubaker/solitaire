//! Spider — 10 cascades, 8 foundations, 1 stock, 2 decks (104 cards).
//!
//! Suit count is configurable (1 / 2 / 4); 4-suit is the default and
//! hardest variant. Multi-card tableau moves require a SUITED
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
        Self::four_suit()
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
mod tests {
    use super::*;
    use crate::cards::Suit;
    use crate::session::GameSession;

    #[test]
    fn deal_distributes_104_cards_correctly() {
        let s = GameSession::new(Spider::four_suit(), 1);
        let cascade_total: usize = (CASCADE_FIRST..=CASCADE_LAST)
            .map(|id| s.piles.get(id).len())
            .sum();
        assert_eq!(cascade_total, 54);
        assert_eq!(s.piles.get(STOCK).len(), 50);
        for id in CASCADE_FIRST..=CASCADE_FIRST + 3 {
            assert_eq!(s.piles.get(id).len(), 6);
        }
        for id in CASCADE_FIRST + 4..=CASCADE_LAST {
            assert_eq!(s.piles.get(id).len(), 5);
        }
        // Top of every cascade face-up.
        for id in CASCADE_FIRST..=CASCADE_LAST {
            assert!(s.piles.get(id).top().unwrap().face_up);
        }
    }

    #[test]
    fn cascades_use_uniform_fanned_spacing() {
        let rules = Spider::four_suit();
        let slots = rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT);
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            assert_eq!(slots[cid as usize].layout, PileLayout::FannedDown);
        }
    }

    #[test]
    fn descending_any_suit_legal_single_card_to_cascade() {
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Six).face_up());
        piles
            .get_mut(CASCADE_FIRST + 1)
            .cards
            .push(Card::new(Suit::Spades, Rank::Seven).face_up());
        // 6♥ onto 7♠ is legal in Spider (suit doesn't matter for single-card moves).
        let m = Move::simple(CASCADE_FIRST, 1, CASCADE_FIRST + 1);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn multi_card_move_requires_suited_tail() {
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        let src = CASCADE_FIRST;
        // 7♠ 6♥ — descending but mixed suit → multi-card move illegal.
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Seven).face_up());
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Six).face_up());
        let dst = CASCADE_FIRST + 1;
        piles
            .get_mut(dst)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Eight).face_up());
        let m = Move::simple(src, 2, dst);
        assert!(!rules.legal_move(&piles, &m));
    }

    #[test]
    fn single_click_move_picks_leftmost_legal_destination() {
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Clubs, Rank::King).face_up());
        }

        let src = CASCADE_FIRST + 5;
        piles.get_mut(src).cards.clear();
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Six).face_up());
        let left_dst = CASCADE_FIRST + 2;
        let right_dst = CASCADE_FIRST + 4;
        for dst in [left_dst, right_dst] {
            piles.get_mut(dst).cards.clear();
            piles
                .get_mut(dst)
                .cards
                .push(Card::new(Suit::Hearts, Rank::Seven).face_up());
        }

        let m = rules
            .single_click_move(&piles, src, 0)
            .expect("six can move onto two sevens");
        assert_eq!(m.from, src);
        assert_eq!(m.to, left_dst);
        assert_eq!(m.take, 1);
    }

    #[test]
    fn single_click_move_prefers_longest_destination_run() {
        let rules = Spider::one_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Spades, Rank::King).face_up());
        }

        let src = CASCADE_FIRST + 8;
        piles.get_mut(src).cards.clear();
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Six).face_up());

        let left_short_dst = CASCADE_FIRST + 1;
        piles.get_mut(left_short_dst).cards.clear();
        piles
            .get_mut(left_short_dst)
            .cards
            .push(Card::new(Suit::Spades, Rank::Seven).face_up());

        let right_long_dst = CASCADE_FIRST + 5;
        piles.get_mut(right_long_dst).cards.clear();
        for r in [Rank::Nine, Rank::Eight, Rank::Seven] {
            piles
                .get_mut(right_long_dst)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }

        let m = rules
            .single_click_move(&piles, src, 0)
            .expect("six can move onto either seven");
        assert_eq!(m.from, src);
        assert_eq!(m.to, right_long_dst);
        assert_eq!(m.take, 1);
    }

    #[test]
    fn complete_run_auto_collapses_to_foundation() {
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        // Build a K→A suited spades run on cascade 0.
        let cid = CASCADE_FIRST;
        for r in [
            Rank::King,
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
            Rank::Eight,
            Rank::Seven,
            Rank::Six,
            Rank::Five,
            Rank::Four,
            Rank::Three,
            Rank::Two,
            Rank::Ace,
        ] {
            piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        let m = rules.after_move(&piles).expect("complete run detected");
        assert_eq!(m.from, cid);
        assert_eq!(m.to, FOUND_FIRST);
        assert_eq!(m.take, 13);
    }

    #[test]
    fn suited_multi_card_move_to_empty_cascade_is_legal() {
        // Regression check: a suited descending tail dragged to an
        // EMPTY cascade should land. (Reported as "I can't move a group
        // of cards to a new pile" — the rule allows it; verifying.)
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        let src = CASCADE_FIRST;
        for r in [Rank::Eight, Rank::Seven, Rank::Six] {
            piles
                .get_mut(src)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        // Cascade 1 is empty.
        let dst = CASCADE_FIRST + 1;
        assert!(piles.get(dst).is_empty());
        let m = Move::simple(src, 3, dst);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn suited_multi_card_move_onto_higher_card_is_legal() {
        // Same suited tail (8♠ 7♠ 6♠) onto a 9 of any suit lands.
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        let src = CASCADE_FIRST;
        for r in [Rank::Eight, Rank::Seven, Rank::Six] {
            piles
                .get_mut(src)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        let dst = CASCADE_FIRST + 1;
        piles
            .get_mut(dst)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Nine).face_up());
        let m = Move::simple(src, 3, dst);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn one_suit_long_descending_tail_relocates_legally() {
        // Mirrors the in-game state the user asked about: a 6-down-to-A
        // suited spades run on cascade 0, an 8-7-6 spades face-up tail
        // on cascade 1. Moving the 5-A (5 cards) from cascade 0 onto
        // the 6 of cascade 1 is a legal Spider move — it just
        // re-organises an already-suited tail under a higher card. The
        // resulting cascade 1 becomes a clean 8→A suited run.
        let rules = Spider::one_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        let src = CASCADE_FIRST;
        for r in [
            Rank::Six,
            Rank::Five,
            Rank::Four,
            Rank::Three,
            Rank::Two,
            Rank::Ace,
        ] {
            piles
                .get_mut(src)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        let dst = CASCADE_FIRST + 1;
        for r in [Rank::Eight, Rank::Seven, Rank::Six] {
            piles
                .get_mut(dst)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        // Move the 5-A tail (5 cards) onto the 6.
        let m = Move::simple(src, 5, dst);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn stock_dispense_undoes_in_one_step() {
        // Regression: clicking stock fires `on_pile_click` which
        // returns N_CASCADES moves; before batching, each landed as
        // its own user-undo step and the player had to press Undo
        // ten times to roll back one dispense.
        let mut s = GameSession::new(Spider::four_suit(), 1);
        let stock_before = s.piles.get(STOCK).len();
        let cascade_lens_before: Vec<usize> = (CASCADE_FIRST..=CASCADE_LAST)
            .map(|id| s.piles.get(id).len())
            .collect();

        let moves = s.rules.on_pile_click(&s.piles, STOCK);
        assert_eq!(moves.len(), N_CASCADES, "stock dispenses one per cascade");
        assert!(s.try_apply_batch(moves), "dispense applies");
        assert_eq!(s.piles.get(STOCK).len(), stock_before - N_CASCADES);
        for (i, id) in (CASCADE_FIRST..=CASCADE_LAST).enumerate() {
            assert_eq!(s.piles.get(id).len(), cascade_lens_before[i] + 1);
        }

        // ONE undo rolls back the entire 10-card dispense.
        assert!(s.try_undo());
        assert_eq!(s.piles.get(STOCK).len(), stock_before);
        for (i, id) in (CASCADE_FIRST..=CASCADE_LAST).enumerate() {
            assert_eq!(s.piles.get(id).len(), cascade_lens_before[i]);
        }
    }

    #[test]
    fn foundation_collapse_flips_newly_exposed_facedown() {
        // Regression: a K→A run collapse left the freshly exposed
        // face-down card face-down, leaving the cascade visually stuck
        // until the player did something else to trigger another move.
        let mut s = GameSession::new(Spider::four_suit(), 1);
        s.piles = PileSet::from_slots(
            &Spider::four_suit().pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let cid = CASCADE_FIRST;
        // Bottom of cascade: a face-down "buried" card.
        s.piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Five));
        // Above it: a K-down-to-2 suited spades run.
        for r in [
            Rank::King,
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
            Rank::Eight,
            Rank::Seven,
            Rank::Six,
            Rank::Five,
            Rank::Four,
            Rank::Three,
            Rank::Two,
        ] {
            s.piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        // Park the Ace on cascade 1.
        let src = CASCADE_FIRST + 1;
        s.piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ace).face_up());
        // User move: Ace onto the 2. K→A collapse fires.
        let m = Move::simple(src, 1, cid);
        assert!(s.try_apply(m));
        // Cascade now has 1 card (the previously-buried 5♥). It must
        // be face-up after the collapse.
        assert_eq!(s.piles.get(cid).len(), 1);
        let top = s.piles.get(cid).top().unwrap();
        assert_eq!(top.rank, Rank::Five);
        assert_eq!(top.suit, Suit::Hearts);
        assert!(
            top.face_up,
            "Spider auto-collapse must flip the newly exposed card"
        );
    }

    #[test]
    fn complete_run_collapse_chain_after_user_move() {
        // Integration check: place an Ace on top of a Q-down-to-2
        // suited run via `try_apply`. The K we'll set up first.
        // Expected: after_move detects the K→A run and auto-collapses.
        let mut s = GameSession::new(Spider::four_suit(), 1);
        // Replace the dealt state with a curated cascade.
        s.piles = PileSet::from_slots(
            &Spider::four_suit().pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let cid = CASCADE_FIRST;
        // Push K..2 (face-up).
        for r in [
            Rank::King,
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
            Rank::Eight,
            Rank::Seven,
            Rank::Six,
            Rank::Five,
            Rank::Four,
            Rank::Three,
            Rank::Two,
        ] {
            s.piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        // Park the Ace on cascade 1.
        let src = CASCADE_FIRST + 1;
        s.piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ace).face_up());
        // User move: Ace onto the 2. Should trigger the K→A collapse
        // via `after_move`.
        let m = Move::simple(src, 1, cid);
        assert!(s.try_apply(m), "user A→2 move accepted");
        // After collapse: cascade 0 empty, foundation 0 has 13 cards.
        assert!(s.piles.get(cid).is_empty(), "K-A run collapsed off cascade");
        assert_eq!(s.piles.get(FOUND_FIRST).len(), 13);
    }

    #[test]
    fn recording_reports_complete_run_collapse() {
        let mut s = GameSession::new(Spider::four_suit(), 1);
        s.piles = PileSet::from_slots(
            &Spider::four_suit().pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let cid = CASCADE_FIRST;
        for r in [
            Rank::King,
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
            Rank::Eight,
            Rank::Seven,
            Rank::Six,
            Rank::Five,
            Rank::Four,
            Rank::Three,
            Rank::Two,
        ] {
            s.piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        let src = CASCADE_FIRST + 1;
        s.piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ace).face_up());

        let records = s
            .try_apply_recording(Move::simple(src, 1, cid))
            .expect("A onto 2 applies and collapses");
        assert_eq!(records.len(), 2);
        assert!(!records[0].is_auto);
        assert!(records[1].is_auto);
        assert_eq!(records[1].m.from, cid);
        assert_eq!(records[1].m.to, FOUND_FIRST);
        assert_eq!(records[1].m.take, 13);
    }

    #[test]
    fn stock_click_blocked_when_any_cascade_empty() {
        let rules = Spider::four_suit();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        for _ in 0..50 {
            piles
                .get_mut(STOCK)
                .cards
                .push(Card::new(Suit::Spades, Rank::Two));
        }
        // Cascade 0 left empty → click should yield no moves.
        for cid in CASCADE_FIRST + 1..=CASCADE_LAST {
            piles
                .get_mut(cid)
                .cards
                .push(Card::new(Suit::Spades, Rank::Two).face_up());
        }
        assert!(rules.on_pile_click(&piles, STOCK).is_empty());
    }
}
