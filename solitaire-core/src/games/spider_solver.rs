//! In-process Spider solver used by the Debug → Generate Spider
//! Seeds menu action. Runs iterative DFS over a `PileSet` snapshot,
//! folds forced moves (auto-collapse K→A runs) into the apply step
//! so they don't burn search depth, prunes obviously sterile moves,
//! and tracks visited states in a transposition table so the same
//! position is never re-explored.
//!
//! Performance is good enough to classify most seeds in well under
//! a second; the long tail (genuinely hard 4-suit boards) gets
//! capped by the caller's time + node budget and reported as
//! `Timeout`. The seed generator only adds seeds for which the
//! solver returns `Won`, so timeouts are silently dropped from the
//! pool — the cost is that a few winnable-but-hard deals never make
//! the bundled list.

use std::collections::HashSet;

use crate::cards::{Card, Rank};
use crate::piles::{PileId, PileSet};
use crate::session::{apply_move, Move};

const FOUND_FIRST: PileId = 0;
const FOUND_LAST: PileId = 7;
const STOCK: PileId = 8;
const CASCADE_FIRST: PileId = 9;
const CASCADE_LAST: PileId = 18;
const N_CASCADES: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveResult {
    Won,
    /// Search ran to exhaustion without finding a win — the deal is
    /// proven unwinnable from the starting position.
    Exhausted,
    /// Hit the wall-clock or node budget first; the deal is not
    /// proven unwinnable, just not solved within the budget.
    Timeout,
}

#[derive(Clone, Copy, Debug)]
pub struct SolverBudget {
    pub deadline: web_time::Instant,
    pub max_nodes: u64,
}

impl SolverBudget {
    pub fn from_duration(duration: std::time::Duration, max_nodes: u64) -> Self {
        Self {
            deadline: web_time::Instant::now() + duration,
            max_nodes,
        }
    }
}

pub fn solve(piles: &PileSet, budget: SolverBudget) -> SolveResult {
    let mut state = piles.clone();
    while let Some(m) = collapse_step(&state) {
        apply_move(&mut state, &m);
    }
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
            frame.moves = generate_ordered_moves(&frame.state);
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
        while let Some(am) = collapse_step(&child_state) {
            apply_move(&mut child_state, &am);
        }
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

fn collapse_step(piles: &PileSet) -> Option<Move> {
    for cid in CASCADE_FIRST..=CASCADE_LAST {
        let cascade = piles.get(cid);
        if !has_complete_run_on_top(cascade) {
            continue;
        }
        for fid in FOUND_FIRST..=FOUND_LAST {
            if piles.get(fid).is_empty() {
                let mut m = Move::simple(cid, 13, fid);
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

fn generate_ordered_moves(piles: &PileSet) -> Vec<Move> {
    let mut scored: Vec<(i32, Move)> = Vec::new();
    for src_id in CASCADE_FIRST..=CASCADE_LAST {
        let src = piles.get(src_id);
        let n = src.cards.len();
        if n == 0 {
            continue;
        }
        for start_idx in 0..n {
            if !src.cards[start_idx].face_up {
                continue;
            }
            let tail = &src.cards[start_idx..];
            let take = tail.len();
            if take > 1 && !is_suited_run(tail) {
                continue;
            }
            let head = &tail[0];
            for dst_id in CASCADE_FIRST..=CASCADE_LAST {
                if dst_id == src_id {
                    continue;
                }
                let dst = piles.get(dst_id);
                let legal = match dst.top() {
                    None => true,
                    Some(top) => top.face_up && Some(head.rank) == top.rank.next_down(),
                };
                if !legal {
                    continue;
                }
                let exposes = start_idx > 0 && !src.cards[start_idx - 1].face_up;
                let creates_suited = dst.top().is_some_and(|t| {
                    t.face_up && t.suit == head.suit && Some(head.rank) == t.rank.next_down()
                });
                let destroys_suited = start_idx > 0 && {
                    let pred = &src.cards[start_idx - 1];
                    pred.face_up
                        && pred.suit == head.suit
                        && Some(head.rank) == pred.rank.next_down()
                };
                // Pure-shuffle prune: identical suited parent before
                // and after — the move doesn't change the
                // junction-count and bloats the TT.
                if !exposes && creates_suited && destroys_suited {
                    continue;
                }
                let mut score = 0i32;
                if exposes {
                    score += 1000;
                }
                if creates_suited {
                    score += 100;
                }
                if destroys_suited {
                    score -= 200;
                }
                score += take as i32 * 5;
                let mut m = Move::simple(src_id, take as u8, dst_id);
                if exposes {
                    m = m.with_flip_source();
                }
                scored.push((score, m));
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<Move> = scored.into_iter().map(|(_, m)| m).collect();
    if piles.get(STOCK).len() >= N_CASCADES {
        for col in 0..N_CASCADES {
            out.push(Move::simple(STOCK, 1, CASCADE_FIRST + col as u8).with_flip_moved());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::spider::Spider;
    use crate::games::GameRules;
    use crate::session::GameSession;

    #[test]
    fn solver_recognises_pre_won_board() {
        let mut s = GameSession::new(Spider::four_suit(), 1);
        for fid in FOUND_FIRST..=FOUND_LAST {
            s.piles.get_mut(fid).cards.clear();
            for _ in 0..13 {
                s.piles
                    .get_mut(fid)
                    .cards
                    .push(crate::cards::Card::new(
                        crate::cards::Suit::Spades,
                        Rank::King,
                    ));
            }
        }
        let budget = SolverBudget::from_duration(std::time::Duration::from_millis(50), 1_000);
        assert_eq!(solve(&s.piles, budget), SolveResult::Won);
    }

    #[test]
    fn solver_returns_exhausted_on_locked_board() {
        let rules = Spider::four_suit();
        let mut piles = crate::piles::PileSet::from_slots(
            &rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            piles
                .get_mut(cid)
                .cards
                .push(crate::cards::Card::new(crate::cards::Suit::Spades, Rank::King).face_up());
        }
        let budget = SolverBudget::from_duration(std::time::Duration::from_secs(1), 100_000);
        assert_eq!(solve(&piles, budget), SolveResult::Exhausted);
    }
}
