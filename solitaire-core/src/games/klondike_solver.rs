//! In-process Klondike solver — iterative DFS + transposition
//! table, same shape as the Spider solver. Generates legal moves
//! for the four families (tableau→tableau, tableau→foundation,
//! waste→tableau / waste→foundation, stock click), prioritises
//! face-down exposure + foundation progression, and uses
//! `apply_move` directly so it doesn't drift from the live game's
//! rules. Wired into the Debug → Generate Klondike Seeds menu
//! action.
//!
//! Solves 1-draw and 3-draw Klondike (the `Klondike::draw_count`
//! the caller passes flows through into the dealt session).
//! Per Solvitaire's published winnability numbers, ~82 % of
//! 1-draw deals are winnable and the solver classifies most of
//! them under a second; the long tail caps at the caller's
//! deadline + node budget.

use std::collections::HashSet;

use crate::cards::{Card, Rank};
use crate::piles::{PileId, PileSet};
use crate::session::{apply_move, Move};

const STOCK: PileId = 0;
const WASTE: PileId = 1;
const FOUND_FIRST: PileId = 2;
const FOUND_LAST: PileId = 5;
const TABLEAU_FIRST: PileId = 6;
const TABLEAU_LAST: PileId = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveResult {
    Won,
    Exhausted,
    Timeout,
}

#[derive(Clone, Copy, Debug)]
pub struct SolverBudget {
    pub deadline: web_time::Instant,
    pub max_nodes: u64,
    pub draw_count: u8,
}

impl SolverBudget {
    pub fn from_duration(duration: std::time::Duration, max_nodes: u64, draw_count: u8) -> Self {
        Self {
            deadline: web_time::Instant::now() + duration,
            max_nodes,
            draw_count,
        }
    }
}

pub fn solve(piles: &PileSet, budget: SolverBudget) -> SolveResult {
    let state = piles.clone();
    if is_won(&state) {
        return SolveResult::Won;
    }
    let mut visited: HashSet<u64> = HashSet::new();
    visited.insert(hash_state(&state));
    let mut stack: Vec<Frame> = Vec::with_capacity(256);
    stack.push(Frame {
        state,
        moves: vec![],
        next_move_idx: 0,
        generated: false,
    });

    let mut nodes = 0u64;
    while let Some(frame) = stack.last_mut() {
        if !frame.generated {
            frame.moves = generate_ordered_moves(&frame.state, budget.draw_count);
            frame.generated = true;
        }
        if frame.next_move_idx >= frame.moves.len() {
            stack.pop();
            continue;
        }
        nodes += 1;
        if nodes > budget.max_nodes {
            return SolveResult::Timeout;
        }
        if nodes & 4095 == 0 && web_time::Instant::now() >= budget.deadline {
            return SolveResult::Timeout;
        }
        let m = frame.moves[frame.next_move_idx];
        frame.next_move_idx += 1;
        let mut child_state = frame.state.clone();
        apply_move(&mut child_state, &m);
        if is_won(&child_state) {
            return SolveResult::Won;
        }
        let ch = hash_state(&child_state);
        if !visited.insert(ch) {
            continue;
        }
        stack.push(Frame {
            state: child_state,
            moves: vec![],
            next_move_idx: 0,
            generated: false,
        });
    }
    SolveResult::Exhausted
}

struct Frame {
    state: PileSet,
    moves: Vec<Move>,
    next_move_idx: usize,
    generated: bool,
}

fn is_won(piles: &PileSet) -> bool {
    for fid in FOUND_FIRST..=FOUND_LAST {
        if piles.get(fid).cards.len() != 13 {
            return false;
        }
    }
    true
}

fn hash_state(piles: &PileSet) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    for pile in piles.iter() {
        pile.cards.len().hash(&mut h);
        pile.cards.hash(&mut h);
    }
    h.finish()
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

fn legal_to_foundation(piles: &PileSet, card: &Card) -> Option<PileId> {
    for fid in FOUND_FIRST..=FOUND_LAST {
        let f = piles.get(fid);
        match f.top() {
            None => {
                if card.rank == Rank::Ace {
                    return Some(fid);
                }
            }
            Some(top) => {
                if same_suit_ascending(top, card) {
                    return Some(fid);
                }
            }
        }
    }
    None
}

fn generate_ordered_moves(piles: &PileSet, draw_count: u8) -> Vec<Move> {
    let mut scored: Vec<(i32, Move)> = Vec::new();

    // 1) Tableau-top → foundation.
    for tid in TABLEAU_FIRST..=TABLEAU_LAST {
        let pile = piles.get(tid);
        let Some(top) = pile.top() else { continue };
        if !top.face_up {
            continue;
        }
        if let Some(fid) = legal_to_foundation(piles, top) {
            let exposes = pile.cards.len() > 1 && !pile.cards[pile.cards.len() - 2].face_up;
            let mut m = Move::simple(tid, 1, fid);
            if exposes {
                m = m.with_flip_source();
            }
            // Foundation moves are nearly always good — high score
            // so the DFS commits to them first.
            scored.push((2000, m));
        }
    }

    // 2) Waste top → foundation.
    {
        let waste = piles.get(WASTE);
        if let Some(top) = waste.top() {
            if let Some(fid) = legal_to_foundation(piles, top) {
                scored.push((1900, Move::simple(WASTE, 1, fid)));
            }
        }
    }

    // 3) Tableau face-up run → tableau (alt-color descending).
    for src_id in TABLEAU_FIRST..=TABLEAU_LAST {
        let src = piles.get(src_id);
        let n = src.cards.len();
        for start_idx in 0..n {
            if !src.cards[start_idx].face_up {
                continue;
            }
            let tail = &src.cards[start_idx..];
            if !is_valid_run(tail) {
                continue;
            }
            let take = tail.len();
            let head = &tail[0];
            let exposes = start_idx > 0 && !src.cards[start_idx - 1].face_up;
            let empties = start_idx == 0;
            for dst_id in TABLEAU_FIRST..=TABLEAU_LAST {
                if dst_id == src_id {
                    continue;
                }
                let dst = piles.get(dst_id);
                let legal = match dst.top() {
                    None => head.rank == Rank::King,
                    Some(top) => top.face_up && alt_color_descending(top, head),
                };
                if !legal {
                    continue;
                }
                // Skip moving a King-led full column into another
                // empty column — pure shuffle.
                if empties && dst.is_empty() && head.rank == Rank::King {
                    continue;
                }
                let mut score = 0i32;
                if exposes {
                    score += 1000;
                }
                score += take as i32 * 3;
                let mut m = Move::simple(src_id, take as u8, dst_id);
                if exposes {
                    m = m.with_flip_source();
                }
                scored.push((score, m));
            }
        }
    }

    // 4) Waste top → tableau.
    {
        let waste = piles.get(WASTE);
        if let Some(top) = waste.top() {
            for dst_id in TABLEAU_FIRST..=TABLEAU_LAST {
                let dst = piles.get(dst_id);
                let legal = match dst.top() {
                    None => top.rank == Rank::King,
                    Some(dt) => dt.face_up && alt_color_descending(dt, top),
                };
                if legal {
                    scored.push((500, Move::simple(WASTE, 1, dst_id)));
                }
            }
        }
    }

    // 5) Stock click — last priority because it doesn't change
    // piles' face-up content, just shuffles stock↔waste.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<Move> = scored.into_iter().map(|(_, m)| m).collect();

    let stock = piles.get(STOCK);
    let waste = piles.get(WASTE);
    if !stock.is_empty() {
        let n = (stock.len() as u8).min(draw_count.max(1));
        out.push(Move::simple(STOCK, n, WASTE).with_flip_moved());
    } else if !waste.is_empty() {
        out.push(
            Move::simple(WASTE, waste.cards.len() as u8, STOCK)
                .with_flip_moved()
                .with_reverse(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::klondike::Klondike;
    use crate::session::GameSession;

    #[test]
    fn solver_recognises_pre_won_board() {
        let mut s = GameSession::new(Klondike::with_draw_count(1), 1);
        for fid in FOUND_FIRST..=FOUND_LAST {
            s.piles.get_mut(fid).cards.clear();
            for _ in 0..13 {
                s.piles.get_mut(fid).cards.push(crate::cards::Card::new(
                    crate::cards::Suit::Spades,
                    Rank::King,
                ));
            }
        }
        let budget =
            SolverBudget::from_duration(std::time::Duration::from_millis(50), 1_000, 1);
        assert_eq!(solve(&s.piles, budget), SolveResult::Won);
    }
}
