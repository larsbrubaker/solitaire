//! Klondike solitaire — 7 tableau columns, 4 foundations, 1 stock, 1 waste.
//! Draw count is configurable (1 by default, 3 for the Microsoft "Classic"
//! 3-card-draw variant). Both modes share slug `klondike`.

use rand::rngs::StdRng;

use crate::cards::{Card, Rank};
use crate::consts::{COL_PITCH, PLAYFIELD_LEFT, TABLEAU_BASE_Y, TOP_ROW_BOTTOM_Y, WASTE_FAN_DX};
use crate::piles::{PileId, PileKind, PileLayout, PileSet, PileSlot};
use crate::session::Move;

use super::GameRules;

// Pile ids:
const STOCK: PileId = 0;
const WASTE: PileId = 1;
const FOUND_FIRST: PileId = 2;
const FOUND_LAST: PileId = 5;
const TABLEAU_FIRST: PileId = 6;
const TABLEAU_LAST: PileId = 12;

const fn slots() -> [PileSlot; 13] {
    [
        PileSlot {
            id: STOCK,
            kind: PileKind::Stock,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: WASTE,
            kind: PileKind::Waste,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        // Foundations occupy columns 3..6 of the top row.
        PileSlot {
            id: 2,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + 3.0 * COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: 3,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + 4.0 * COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: 4,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + 5.0 * COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        PileSlot {
            id: 5,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: PLAYFIELD_LEFT + 6.0 * COL_PITCH,
            origin_y: TOP_ROW_BOTTOM_Y,
        },
        // Tableau columns 0..6.
        PileSlot {
            id: 6,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 7,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 8,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 2.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 9,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 3.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 10,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 4.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 11,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 5.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
        PileSlot {
            id: 12,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: PLAYFIELD_LEFT + 6.0 * COL_PITCH,
            origin_y: TABLEAU_BASE_Y,
        },
    ]
}

static SLOTS: [PileSlot; 13] = slots();

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

fn is_tableau(id: PileId) -> bool {
    (TABLEAU_FIRST..=TABLEAU_LAST).contains(&id)
}

fn is_foundation(id: PileId) -> bool {
    (FOUND_FIRST..=FOUND_LAST).contains(&id)
}

/// Are `top` and `cand` an alternating-color, descending pair? `cand` is
/// the card being placed on top of `top`.
fn alt_color_descending(top: &Card, cand: &Card) -> bool {
    if top.suit.color() == cand.suit.color() {
        return false;
    }
    Some(cand.rank) == top.rank.next_down()
}

/// Same suit, ascending pair: `cand` is being placed on top of `top` in a
/// foundation pile.
fn same_suit_ascending(top: &Card, cand: &Card) -> bool {
    top.suit == cand.suit && Some(cand.rank) == top.rank.next_up()
}

/// Check that `cards` form a valid alternating-color descending run.
/// All cards must be face-up. Used to validate multi-card tableau moves.
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

impl GameRules for Klondike {
    fn pile_layout(&self) -> &'static [PileSlot] {
        &SLOTS
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

        // 3-card-draw mode: keep the most-recent draw_count cards visible
        // as a fan on the waste, like the Microsoft "Classic" UX.
        if self.draw_count > 1 {
            let waste = piles.get_mut(WASTE);
            waste.fan_top_n = self.draw_count;
            waste.fan_dx = WASTE_FAN_DX;
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
                }];
            }
        }
        Vec::new()
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
        let mut piles = PileSet::from_slots(rules.pile_layout());
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
        let mut piles = PileSet::from_slots(rules.pile_layout());
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
        let mut piles = PileSet::from_slots(rules.pile_layout());
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
        let mut piles = PileSet::from_slots(rules.pile_layout());
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
        let mut piles = PileSet::from_slots(rules.pile_layout());
        piles
            .get_mut(WASTE)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
        let m = Move::simple(WASTE, 1, TABLEAU_FIRST);
        assert!(rules.legal_move(&piles, &m));
    }

    #[test]
    fn stock_click_when_nonempty_dispenses_one_to_waste() {
        let rules = Klondike::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
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
    fn stock_click_when_empty_recycles_waste() {
        let rules = Klondike::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
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
