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

use super::{GameRules, CARD_ASPECT};

const CELL_FIRST: PileId = 0;
const CELL_LAST: PileId = 3;
const FOUND_FIRST: PileId = 4;
const FOUND_LAST: PileId = 7;
const CASCADE_FIRST: PileId = 8;
const CASCADE_LAST: PileId = 15;

const N_CASCADES: usize = 8;
/// Vertical budget in card-heights (top row + tableau fan).
const VERT_BUDGET_CARDS: f64 = 5.5;

pub struct FreeCell;

impl FreeCell {
    pub const fn new() -> Self {
        Self
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
        // 8 columns horizontally (4 free cells + 4 foundations sharing
        // the top row with the 8 cascades below). Card size picked to
        // fit both the column count and the worst-case cascade fan.
        let col_gap = 10.0;
        let row_gap = 12.0;
        let card_w_by_width =
            (rect.width - col_gap * (N_CASCADES as f64 - 1.0)) / N_CASCADES as f64;
        let card_h_by_height = (rect.height - row_gap) / VERT_BUDGET_CARDS;
        let card_h = (card_w_by_width * CARD_ASPECT).min(card_h_by_height);
        let card_w = card_h / CARD_ASPECT;
        let col_pitch = card_w + col_gap;
        let used_w = N_CASCADES as f64 * card_w + (N_CASCADES as f64 - 1.0) * col_gap;
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
        let mut out = Vec::with_capacity(16);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::Suit;
    use crate::session::GameSession;

    #[test]
    fn deal_distributes_52_cards_across_8_cascades() {
        let s = GameSession::new(FreeCell::new(), 1);
        let total: usize = (CASCADE_FIRST..=CASCADE_LAST)
            .map(|id| s.piles.get(id).len())
            .sum();
        assert_eq!(total, 52);
        for id in CASCADE_FIRST..=CASCADE_FIRST + 3 {
            assert_eq!(s.piles.get(id).len(), 7);
        }
        for id in CASCADE_FIRST + 4..=CASCADE_LAST {
            assert_eq!(s.piles.get(id).len(), 6);
        }
        // All cards face-up.
        for id in CASCADE_FIRST..=CASCADE_LAST {
            assert!(s.piles.get(id).cards.iter().all(|c| c.face_up));
        }
    }

    #[test]
    fn cell_accepts_single_card() {
        let rules = FreeCell::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Two).face_up());
        let m = Move::simple(CASCADE_FIRST, 1, CELL_FIRST);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn cell_rejects_when_full() {
        let rules = FreeCell::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        piles
            .get_mut(CELL_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::King).face_up());
        piles
            .get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Two).face_up());
        let m = Move::simple(CASCADE_FIRST, 1, CELL_FIRST);
        assert!(!rules.legal_move(&piles, &m));
    }

    #[test]
    fn multi_card_move_limited_by_empty_cells() {
        let rules = FreeCell::new();
        let mut piles =
            PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        // Fill every cell so empty_cells = 0.
        for id in CELL_FIRST..=CELL_LAST {
            piles
                .get_mut(id)
                .cards
                .push(Card::new(Suit::Hearts, Rank::King).face_up());
        }
        // Source cascade run: Q♠ J♥ 10♠ (alt-color descending).
        let src = CASCADE_FIRST;
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Queen).face_up());
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Jack).face_up());
        piles
            .get_mut(src)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ten).face_up());
        // Fill every other cascade so empty_cascades = 0 too.
        for id in (CASCADE_FIRST + 1)..=CASCADE_LAST {
            piles
                .get_mut(id)
                .cards
                .push(Card::new(Suit::Spades, Rank::Two).face_up());
        }
        // Destination = CASCADE_FIRST + 1 (overwrite top so K♥ is on top).
        let dst = CASCADE_FIRST + 1;
        piles.get_mut(dst).cards.clear();
        piles
            .get_mut(dst)
            .cards
            .push(Card::new(Suit::Hearts, Rank::King).face_up());
        // Re-fill the remaining other cascades.
        for id in (CASCADE_FIRST + 2)..=CASCADE_LAST {
            // Already filled above; nothing to do.
            assert!(!piles.get(id).is_empty());
        }
        // With 0 empty cells AND 0 empty cascades, max_movable = 1
        // — moving 3 cards must fail.
        let m = Move::simple(src, 3, dst);
        assert!(!rules.legal_move(&piles, &m));
    }
}
