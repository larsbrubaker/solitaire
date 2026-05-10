//! Mom's Solitaire — a Yukon-style variant with no stock and no waste.
//! All 52 cards land in the seven tableau columns at deal time, and
//! you build the four foundations Ace → King by suit. The big
//! difference from Klondike is movement: any face-up card (along with
//! every card stacked on top of it, regardless of order or colour)
//! can be moved as a unit, as long as the moved card itself drops
//! onto a legal alt-colour-descending target.
//!
//! Named in memory of a Mother's Day 1989 gift — see the Help menu's
//! "Rules" entry for the full story.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::cards::{Card, Rank};
use crate::consts::{COL_PITCH, PLAYFIELD_LEFT, TABLEAU_BASE_Y, TOP_ROW_BOTTOM_Y};
use crate::piles::{PileId, PileKind, PileLayout, PileSet, PileSlot};
use crate::session::Move;

use super::GameRules;

const FOUND_FIRST: PileId = 0;
const FOUND_LAST: PileId = 3;
const TABLEAU_FIRST: PileId = 4;
const TABLEAU_LAST: PileId = 10;

const fn slots() -> [PileSlot; 11] {
    [
        // Foundations occupy the top-left four columns (no stock/waste
        // in this variant, so the slots that Klondike uses for those
        // are vacant — we move the foundations leftward).
        PileSlot {
            id: 0,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: 1,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: 2,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + 2.0 * COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: 3,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + 3.0 * COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        // Tableau columns 0..6.
        PileSlot {
            id: 4,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 5,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 6,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 2.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 7,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 3.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 8,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 4.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 9,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 5.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 10,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 6.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
    ]
}

static SLOTS: [PileSlot; 11] = slots();

#[derive(Default)]
pub struct MomsSolitaire;

impl MomsSolitaire {
    pub const fn new() -> Self {
        Self
    }
}

fn is_tableau(id: PileId) -> bool {
    (TABLEAU_FIRST..=TABLEAU_LAST).contains(&id)
}
fn is_foundation(id: PileId) -> bool {
    (FOUND_FIRST..=FOUND_LAST).contains(&id)
}

fn alt_color_descending(top: &Card, cand: &Card) -> bool {
    if top.suit.color() == cand.suit.color() {
        return false;
    }
    Some(cand.rank) == top.rank.next_down()
}

fn same_suit_ascending(top: &Card, cand: &Card) -> bool {
    top.suit == cand.suit && Some(cand.rank) == top.rank.next_up()
}

impl GameRules for MomsSolitaire {
    fn pile_layout(&self) -> &'static [PileSlot] {
        &SLOTS
    }

    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng) {
        let mut deck = crate::cards::standard_deck();
        deck.shuffle(rng);
        let mut iter = deck.into_iter();

        // Yukon-style deal: column 0 gets exactly one face-up card;
        // columns 1..6 each get `col` face-down cards followed by 5
        // face-up cards. Totals: 1 + 6 + 7 + 8 + 9 + 10 + 11 = 52,
        // so the entire deck lands in the tableau.
        for col in 0u8..7 {
            let n_down = col as usize;
            let n_up = if col == 0 { 1 } else { 5 };
            for _ in 0..n_down {
                let card = iter.next().expect("52-card deck");
                piles.get_mut(TABLEAU_FIRST + col).cards.push(card);
            }
            for _ in 0..n_up {
                let mut card = iter.next().expect("52-card deck");
                card.face_up = true;
                piles.get_mut(TABLEAU_FIRST + col).cards.push(card);
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
        if from.cards.len() < m.take as usize {
            return false;
        }
        let take = m.take as usize;
        let moved = &from.cards[from.cards.len() - take..];
        // All moved cards must be face-up. The Yukon-style permissive
        // rule applies only to face-up cards; you still can't grab a
        // face-down card.
        if moved.iter().any(|c| !c.face_up) {
            return false;
        }

        // Foundation destination — take exactly one and either start
        // with an Ace or extend the existing same-suit run.
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

        // Tableau destination — Yukon-rule: only the BOTTOM moved
        // card has to match the destination's top in alt-colour
        // descending. The cards stacked above it can be in any
        // order; that's what makes Mom's Solitaire feel different
        // from Klondike.
        if is_tableau(m.to) {
            let head = &moved[0];
            return match to.top() {
                None => head.rank == Rank::King,
                Some(top) => alt_color_descending(top, head),
            };
        }

        false
    }

    fn auto_complete_step(&self, piles: &PileSet) -> Option<Move> {
        // Auto-complete only kicks in once every tableau card is
        // face-up — at that point the rest of the game is mechanical.
        for id in TABLEAU_FIRST..=TABLEAU_LAST {
            if piles.get(id).cards.iter().any(|c| !c.face_up) {
                return None;
            }
        }
        for src in TABLEAU_FIRST..=TABLEAU_LAST {
            let pile = piles.get(src);
            let Some(top) = pile.top() else { continue };
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
        "moms"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::Suit;
    use crate::session::GameSession;

    #[test]
    fn deal_distributes_all_52_cards_to_tableau() {
        let s = GameSession::new(MomsSolitaire::new(), 7);
        let total: usize = (TABLEAU_FIRST..=TABLEAU_LAST)
            .map(|id| s.piles.get(id).len())
            .sum();
        assert_eq!(total, 52);
        // Foundations empty.
        for fid in FOUND_FIRST..=FOUND_LAST {
            assert_eq!(s.piles.get(fid).len(), 0);
        }
        // Each column's topmost card is face-up.
        for id in TABLEAU_FIRST..=TABLEAU_LAST {
            assert!(s.piles.get(id).top().unwrap().face_up);
        }
        // First tableau column has exactly 1 card; subsequent columns
        // have 6, 7, 8, 9, 10, 11.
        let expected = [1, 6, 7, 8, 9, 10, 11];
        for (i, &n) in expected.iter().enumerate() {
            assert_eq!(s.piles.get(TABLEAU_FIRST + i as u8).len(), n);
        }
    }

    #[test]
    fn unsorted_pile_above_target_card_is_movable() {
        // Build a column where the head (bottom of move) is a 5♥, and
        // above it sits an unrelated jumble: 9♠, 2♦. In Klondike that
        // pile couldn't be moved as a unit. In Mom's Solitaire it can,
        // as long as the destination's top is a 6♣ or 6♠.
        let rules = MomsSolitaire::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Five).face_up());
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Spades, Rank::Nine).face_up());
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Diamonds, Rank::Two).face_up());
        piles
            .get_mut(TABLEAU_FIRST + 1)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Six).face_up());
        let m = Move::simple(TABLEAU_FIRST, 3, TABLEAU_FIRST + 1);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn foundation_only_accepts_topmost_card() {
        let rules = MomsSolitaire::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
        let m = Move::simple(TABLEAU_FIRST, 1, FOUND_FIRST);
        assert!(rules.legal_move(&piles, &m));
        // Multi-card to foundation rejected.
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
        let m = Move::simple(TABLEAU_FIRST, 2, FOUND_FIRST);
        assert!(!rules.legal_move(&piles, &m));
    }
}
