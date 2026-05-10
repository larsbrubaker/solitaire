//! Mom's Solitaire — a port of the Forth solitaire game my cousin
//! Marlin Eller wrote for his mom Margaret on Mother's Day in 1989.
//! The C# / agg-sharp re-implementation lives at
//! `MatterCAD/Submodules/agg-sharp/examples/MomsSolitaire/MomsGame.cs`;
//! this file ports its rules verbatim.
//!
//! It's a Montana-family / Gaps variant:
//!
//! - **Layout:** every card is laid out face-up on a 13×4 grid, one
//!   card per cell, with the four Aces functioning as moveable
//!   "gaps." There is no stock, no waste, no foundation pile —
//!   every cell is part of the game from move one.
//! - **Goal:** arrange each row so columns 0..11 hold K, Q, J, 10,
//!   9, 8, 7, 6, 5, 4, 3, 2 of a single suit (cards play *to the
//!   right*, descending in rank). Column 12 ends up holding the
//!   Ace of that suit.
//! - **Moves are swaps**: clicking on a gap swaps it with the card
//!   to its left's same-suit one-rank-lower partner. We model that
//!   here with `Move::swap_with_top`. The drag-drop UI accepts a
//!   non-Ace card dropped onto an Ace cell, which has the same
//!   effect.
//! - **Column 0 is special:** the gap at column 0 only accepts a
//!   King (any suit). Once filled, that King's suit fixes the row's
//!   target colour.
//! - **Win condition:** every row's columns 0..11 are a same-suit
//!   K-down-to-2 run.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::cards::{Card, Rank, Suit};
use crate::consts::{CARD_H, TOP_ROW_BOTTOM_Y, VIRTUAL_W};
use crate::piles::{PileId, PileKind, PileLayout, PileSet, PileSlot};
use crate::session::Move;

use super::GameRules;

pub const COLS: u8 = 13;
pub const ROWS: u8 = 4;

/// Logical card width for a Mom's cell. The standard 90 px wide card
/// would put 13 columns at 1170 px — wider than `VIRTUAL_W=1024` —
/// so we shrink it to fit. Other variants are unaffected (per-pile
/// `card_w` / `card_h` fields on `Pile`).
pub const MOMS_CARD_W: f64 = 70.0;
pub const MOMS_CARD_H: f64 = 98.0;

const COL_GAP: f64 = 6.0;
const ROW_GAP: f64 = 8.0;
const PITCH_X: f64 = MOMS_CARD_W + COL_GAP;
const PITCH_Y: f64 = MOMS_CARD_H + ROW_GAP;

/// `(X, Y)` → linear `PileId`. Y=0 is the TOP row in Y-up screen
/// coordinates (the row with the highest pile origin Y), so the
/// first row of cards reads K, Q, J… visually from the top down.
pub const fn cell_id(x: u8, y: u8) -> PileId {
    y * COLS + x
}

pub const fn cell_x(id: PileId) -> u8 {
    id % COLS
}
pub const fn cell_y(id: PileId) -> u8 {
    id / COLS
}

/// Y-up origin (bottom-left of the cell) for `(x, y)`. Row 0 is the
/// TOP row visually; subsequent rows step DOWN by `PITCH_Y`.
const fn origin(x: u8, y: u8) -> (f64, f64) {
    let total_w = COLS as f64 * MOMS_CARD_W + (COLS as f64 - 1.0) * COL_GAP;
    let left = (VIRTUAL_W - total_w) / 2.0;
    let ox = left + x as f64 * PITCH_X;
    // CARD_H ≠ MOMS_CARD_H, but we anchor the top row at
    // `TOP_ROW_BOTTOM_Y` so the grid starts where Klondike's stock
    // would. Subsequent rows step downward (smaller Y in Y-up).
    let oy = TOP_ROW_BOTTOM_Y - (CARD_H - MOMS_CARD_H) - y as f64 * PITCH_Y;
    (ox, oy)
}

/// Generate the 52-cell slot table at compile time.
const fn slots() -> [PileSlot; 52] {
    let mut out = [PileSlot {
        id: 0,
        kind: PileKind::Tableau,
        layout: PileLayout::Stacked,
        origin_x: 0.0,
        origin_y: 0.0,
    }; 52];
    let mut y = 0u8;
    while y < ROWS {
        let mut x = 0u8;
        while x < COLS {
            let (ox, oy) = origin(x, y);
            let id = cell_id(x, y);
            out[id as usize] = PileSlot {
                id,
                kind: PileKind::Tableau,
                layout: PileLayout::Stacked,
                origin_x: ox,
                origin_y: oy,
            };
            x += 1;
        }
        y += 1;
    }
    out
}

static SLOTS: [PileSlot; 52] = slots();

#[derive(Default)]
pub struct MomsSolitaire;

impl MomsSolitaire {
    pub const fn new() -> Self {
        Self
    }
}

/// Standard 52-card deck, returned in Suit-major order (Spades A..K,
/// Hearts A..K, Diamonds A..K, Clubs A..K) — the dealer shuffles it.
fn fresh_deck() -> Vec<Card> {
    crate::cards::standard_deck()
        .into_iter()
        .map(|c| c.face_up())
        .collect()
}

/// Same-suit one-rank-lower partner of `c`, or `None` if `c` is an Ace.
fn one_rank_lower_same_suit(c: &Card) -> Option<(Suit, Rank)> {
    c.rank.next_down().map(|r| (c.suit, r))
}

impl GameRules for MomsSolitaire {
    fn pile_layout(&self) -> &'static [PileSlot] {
        &SLOTS
    }

    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng) {
        // Resize every cell to Mom's smaller card geometry first; the
        // generic `PileSet::from_slots` initialised them with
        // `consts::CARD_W / CARD_H`, which would draw 90-px cards into
        // 70-px-wide cells. Also flag each cell so an Ace top-card
        // renders as a gap (drop target) instead of as a playing card.
        for p in piles.iter_mut() {
            p.card_w = MOMS_CARD_W;
            p.card_h = MOMS_CARD_H;
            p.render_ace_as_gap = true;
        }
        let mut deck = fresh_deck();
        deck.shuffle(rng);
        let mut iter = deck.into_iter();
        for y in 0..ROWS {
            for x in 0..COLS {
                let card = iter.next().expect("52-card deck");
                piles.get_mut(cell_id(x, y)).cards.push(card);
            }
        }
        debug_assert!(iter.next().is_none(), "all 52 cards dealt");
    }

    fn legal_move(&self, piles: &PileSet, m: &Move) -> bool {
        // Mom's only operation is the gap-swap.
        if !m.swap_with_top || m.take != 1 {
            return false;
        }
        if m.from == m.to {
            return false;
        }
        let from_card = match piles.get(m.from).top() {
            Some(c) => *c,
            None => return false,
        };
        let to_card = match piles.get(m.to).top() {
            Some(c) => *c,
            None => return false,
        };
        // The destination must be the gap (an Ace); the source is the
        // card the player wants to slot into that gap. (Equivalently:
        // you don't move Aces — you fill them.)
        if to_card.rank != Rank::Ace || from_card.rank == Rank::Ace {
            return false;
        }
        let dst_x = cell_x(m.to);
        let dst_y = cell_y(m.to);
        if dst_x == 0 {
            // Leftmost column: fixes the row's suit to whatever King
            // gets dropped here. Only Kings allowed.
            return from_card.rank == Rank::King;
        }
        // Otherwise: the card just to the LEFT of the gap dictates
        // the partner. We need its same-suit one-rank-lower mate.
        let left = piles.get(cell_id(dst_x - 1, dst_y));
        let Some(left_card) = left.top() else {
            return false;
        };
        if left_card.rank == Rank::Ace {
            // Two adjacent gaps — nothing fits.
            return false;
        }
        let Some((want_suit, want_rank)) = one_rank_lower_same_suit(left_card) else {
            return false;
        };
        from_card.suit == want_suit && from_card.rank == want_rank
    }

    fn auto_complete_step(&self, _piles: &PileSet) -> Option<Move> {
        None
    }

    fn is_won(&self, piles: &PileSet) -> bool {
        // Each row's columns 0..11 must form a K → 2 same-suit run.
        // Column 12 is allowed to be anything (in practice it's the
        // row's Ace — there's nowhere else for it to go).
        for y in 0..ROWS {
            let suit = match piles.get(cell_id(0, y)).top() {
                Some(c) if c.rank == Rank::King => c.suit,
                _ => return false,
            };
            for x in 1u8..12 {
                let want_value = 13u8 - x;
                let Some(card) = piles.get(cell_id(x, y)).top() else {
                    return false;
                };
                if card.suit != suit || card.rank as u8 != want_value {
                    return false;
                }
            }
        }
        true
    }

    fn game_slug(&self) -> &'static str {
        "moms"
    }
}

/// Is the card at `(x, y)` currently in its final position? A cell is
/// "in order" iff cards `(0..=x, y)` form a same-suit K → (13-x) run.
/// Direct port of `MomsGame.CardIsInOrder` in the C# original.
fn card_is_in_order(board: &[Option<Card>], x: u8, y: u8) -> bool {
    let Some(card) = board[cell_id(x, y) as usize] else {
        return false;
    };
    if (card.rank as u8) != 13 - x {
        return false;
    }
    let suit = card.suit;
    for i in 0..x {
        let Some(o) = board[cell_id(i, y) as usize] else {
            return false;
        };
        if o.suit != suit || (o.rank as u8) != 13 - i {
            return false;
        }
    }
    true
}

/// Generate the list of swaps a shuffle should perform on the current
/// board. Direct port of `MomsGame.Shuffle()` minus the
/// recurse-if-no-move-available retry — the caller is responsible for
/// requesting another shuffle if this one leaves a dead board.
///
/// The returned swaps are intended to be applied in order through the
/// session (`Move::swap(a, b)`) so each lands on the engine's undo
/// stack and `is_won()` runs correctly at the end.
pub fn compute_shuffle_swaps(
    piles: &crate::piles::PileSet,
    rng: &mut StdRng,
) -> Vec<(PileId, PileId)> {
    use rand::Rng;
    // Snapshot the board as a flat array so we can mutate it locally
    // without disturbing the live piles. Each cell holds exactly one
    // card (Mom's invariant) so `Option` is just future-proofing.
    let mut board: Vec<Option<Card>> = (0..(COLS * ROWS))
        .map(|id| piles.get(id).top().copied())
        .collect();
    let mut swaps = Vec::new();
    for y in 0..ROWS {
        for x in 0..COLS {
            let id = cell_id(x, y);
            if card_is_in_order(&board, x, y) {
                continue;
            }
            // Find a random partner that's also out of order. Caps at
            // 100k tries (matching the C# guard); in practice the loop
            // terminates almost immediately because most cells are
            // out of order after the first move.
            let mut other_id = id;
            for _ in 0..100_000 {
                let ox = rng.gen_range(0..COLS);
                let oy = rng.gen_range(0..ROWS);
                if !card_is_in_order(&board, ox, oy) {
                    other_id = cell_id(ox, oy);
                    break;
                }
            }
            if other_id == id {
                continue;
            }
            board.swap(id as usize, other_id as usize);
            swaps.push((id, other_id));
        }
    }
    swaps
}

// ────────────────────────────────────────────────────────────────────
// Click-based UI flow
//
// Mom's Solitaire is played by clicking — never by dragging. The player
// clicks on a gap (Ace cell), and the game finds the unique card that
// matches and swaps it in. The col-0 case is two-step: click the gap to
// arm a wait, then click any King to fill it. Helpers below let
// `GameWidget` resolve a single click into one of three outcomes
// without re-implementing the layout knowledge in the UI layer.
// ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickResolution {
    /// Click did not produce a move and didn't change waiting state.
    Ignored,
    /// Click was on a col-0 gap. UI should set
    /// `model.moms_waiting_king_at = Some(pile)`. Next click on a King
    /// will resolve to `ApplySwap`.
    StartWaitingForKing(PileId),
    /// A swap should be applied to the session. Whether or not the
    /// game was previously in `waiting_for_king`, the UI clears that
    /// state when this fires.
    ApplySwap(Move),
}

/// Resolve a single click. `currently_waiting`, when `Some`, is the
/// pile id of a col-0 gap the player previously clicked.
pub fn resolve_click(
    piles: &PileSet,
    clicked: PileId,
    currently_waiting: Option<PileId>,
) -> ClickResolution {
    let Some(top) = piles.get(clicked).top() else {
        return ClickResolution::Ignored;
    };
    // ── Branch 1: filling a previously-armed col-0 gap ──────────
    if let Some(gap) = currently_waiting {
        if top.rank == Rank::King {
            return ClickResolution::ApplySwap(Move::swap(clicked, gap));
        }
        // Click landed on something that isn't a King — the C# stays
        // in waiting state. Treat as Ignored; the UI keeps the wait.
        return ClickResolution::Ignored;
    }
    // ── Branch 2: clicking on a gap to start a move ─────────────
    if top.rank != Rank::Ace {
        return ClickResolution::Ignored;
    }
    let x = cell_x(clicked);
    let y = cell_y(clicked);
    if x == 0 {
        return ClickResolution::StartWaitingForKing(clicked);
    }
    let left_id = cell_id(x - 1, y);
    let Some(left) = piles.get(left_id).top() else {
        return ClickResolution::Ignored;
    };
    if left.rank == Rank::Ace || left.rank == Rank::Two {
        // Dead gap: would need a value-0 / value-1 card and Aces are
        // gaps not movable cards. No move possible until adjacent
        // moves change the picture.
        return ClickResolution::Ignored;
    }
    let Some((want_suit, want_rank)) = one_rank_lower_same_suit(left) else {
        return ClickResolution::Ignored;
    };
    // Find the matching card in the grid and swap it here.
    for id in 0..(COLS * ROWS) {
        if id == clicked {
            continue;
        }
        if let Some(c) = piles.get(id).top() {
            if c.suit == want_suit && c.rank == want_rank {
                return ClickResolution::ApplySwap(Move::swap(id, clicked));
            }
        }
    }
    ClickResolution::Ignored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::GameSession;

    fn cell(piles: &PileSet, x: u8, y: u8) -> Card {
        *piles.get(cell_id(x, y)).top().expect("cell occupied")
    }

    #[test]
    fn deal_distributes_one_card_per_cell() {
        let s = GameSession::new(MomsSolitaire::new(), 7);
        for y in 0..ROWS {
            for x in 0..COLS {
                assert_eq!(s.piles.get(cell_id(x, y)).len(), 1);
                assert!(cell(&s.piles, x, y).face_up);
            }
        }
        // 52 unique cards.
        let mut seen = std::collections::HashSet::new();
        for y in 0..ROWS {
            for x in 0..COLS {
                let c = cell(&s.piles, x, y);
                assert!(seen.insert((c.suit, c.rank)));
            }
        }
        assert_eq!(seen.len(), 52);
    }

    #[test]
    fn swap_into_gap_must_match_left_neighbour() {
        let rules = MomsSolitaire::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
        for p in piles.iter_mut() {
            p.card_w = MOMS_CARD_W;
            p.card_h = MOMS_CARD_H;
        }
        // Place 5♥ at (3, 0) and an Ace gap at (4, 0).
        piles
            .get_mut(cell_id(3, 0))
            .cards
            .push(Card::new(Suit::Hearts, Rank::Five).face_up());
        piles
            .get_mut(cell_id(4, 0))
            .cards
            .push(Card::new(Suit::Spades, Rank::Ace).face_up());
        // Stash 4♥ at (10, 0) — that's the partner.
        piles
            .get_mut(cell_id(10, 0))
            .cards
            .push(Card::new(Suit::Hearts, Rank::Four).face_up());
        // Stash 4♣ at (11, 0) — same rank, wrong suit.
        piles
            .get_mut(cell_id(11, 0))
            .cards
            .push(Card::new(Suit::Clubs, Rank::Four).face_up());

        let from_4h = cell_id(10, 0);
        let from_4c = cell_id(11, 0);
        let to_gap = cell_id(4, 0);

        assert!(rules.legal_move(&piles, &Move::swap(from_4h, to_gap)));
        // Wrong-suit partner is rejected.
        assert!(!rules.legal_move(&piles, &Move::swap(from_4c, to_gap)));
    }

    #[test]
    fn col_zero_only_accepts_kings() {
        let rules = MomsSolitaire::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
        for p in piles.iter_mut() {
            p.card_w = MOMS_CARD_W;
            p.card_h = MOMS_CARD_H;
        }
        piles
            .get_mut(cell_id(0, 0))
            .cards
            .push(Card::new(Suit::Diamonds, Rank::Ace).face_up());
        piles
            .get_mut(cell_id(5, 0))
            .cards
            .push(Card::new(Suit::Hearts, Rank::King).face_up());
        piles
            .get_mut(cell_id(6, 0))
            .cards
            .push(Card::new(Suit::Hearts, Rank::Queen).face_up());

        let to_col_zero_gap = cell_id(0, 0);
        // King → col-0 gap is fine, regardless of suit.
        assert!(rules.legal_move(&piles, &Move::swap(cell_id(5, 0), to_col_zero_gap)));
        // Queen → col-0 gap is rejected.
        assert!(!rules.legal_move(&piles, &Move::swap(cell_id(6, 0), to_col_zero_gap)));
    }

    #[test]
    fn ace_is_never_legal_as_a_source() {
        let rules = MomsSolitaire::new();
        let mut piles = PileSet::from_slots(rules.pile_layout());
        for p in piles.iter_mut() {
            p.card_w = MOMS_CARD_W;
            p.card_h = MOMS_CARD_H;
        }
        piles
            .get_mut(cell_id(0, 0))
            .cards
            .push(Card::new(Suit::Diamonds, Rank::Ace).face_up());
        piles
            .get_mut(cell_id(1, 0))
            .cards
            .push(Card::new(Suit::Spades, Rank::Ace).face_up());
        // An Ace can't be moved into another Ace's slot.
        assert!(!rules.legal_move(&piles, &Move::swap(cell_id(0, 0), cell_id(1, 0))));
    }
}
