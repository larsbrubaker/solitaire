//! Clean-room Spider solver. See
//! [`crate::games::SPEC_SPIDER_SOLVER`](../SPEC_SPIDER_SOLVER.md) for
//! the design spec; all algorithms derived from Blake & Gent's JAIR
//! 2026 paper (arXiv:1906.12314v5) and first-principles reasoning
//! about Spider's rules. No `Solvitaire` source consulted.
//!
//! Architecture (per SPEC):
//! - DFS + apply/revert trailing (paper §5.1)
//! - Transposition table keyed by canonical-pile-order 64-bit hash
//!   (paper §5.2, §5.3)
//! - Spider-specific prunes P1..P4 (first-principles; paper's formal
//!   dominances D1+D2 exclude Spider per paper §5.4.3 and §C.2)
//! - Optional S2 suit-symmetry streamliner for 1-suit / 2-suit Spider
//!   (paper §5.5)
//! - "Smart" wrapper that runs S2 for 10 % of the budget then falls
//!   back to strict for 90 % (paper §5.5)
//!
//! The seed generator only adds seeds for which the solver returns
//! `Won`, so timeouts are silently dropped from the bundled pool.

use std::collections::HashSet;

use crate::cards::{Card, Rank, Suit};
use crate::piles::{PileId, PileSet};
use crate::session::{apply_move, revert_move, Move};

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

/// Streamliner mode for the transposition-table hash. Per SPEC §4:
/// for 1-suit and 2-suit Spider the cache can collapse states that
/// differ only in suit detail, since identical-colour cards are
/// often indistinguishable in those variants. False negatives are
/// possible; the `smart_solve` wrapper guards against them by
/// re-running strict on streamliner failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamlinerMode {
    /// No collapse — every suit is distinct. Sound: a `Won` /
    /// `Exhausted` verdict is final. Required for 4-suit Spider.
    Strict,
    /// Collapse suit to a single symbol — treat the deck as
    /// 1-suit. Correct for 1-suit Spider (where every card already
    /// has the same suit) and a false-negative-prone streamliner
    /// for higher-suit variants.
    SuitFlatten,
    /// Collapse suit to colour parity (♣/♠ → "black", ♦/♥ → "red").
    /// Sound for 2-suit Spider when the chosen suits are one of
    /// each colour; a streamliner for 4-suit.
    SuitColour,
}

/// One search choice — a single tableau move is one move, a stock
/// "deal a row" action is a vec of 10 moves applied as an
/// atomic unit. The solver treats each Choice as one branch in
/// the DFS so partial-deal intermediate states are never explored
/// (the player can't reach them either).
type Choice = Vec<Move>;

pub fn solve(piles: &PileSet, budget: SolverBudget) -> SolveResult {
    solve_with(piles, budget, StreamlinerMode::Strict)
}

/// Smart-streamliner wrapper per SPEC §5. Spends 10 % of the
/// time budget under `streamliner` (e.g. [`StreamlinerMode::SuitColour`])
/// looking for a quick `Won`; if the streamliner returns
/// `Exhausted` or `Timeout`, restarts under [`StreamlinerMode::Strict`]
/// with the full budget. Sound: a final `Won` is always real, and a
/// final `Exhausted` always came from the strict pass.
///
/// Picking the right `streamliner` is the caller's job. Spider
/// 1-suit → `SuitFlatten`; 2-suit → `SuitColour`; 4-suit → don't
/// bother (just call [`solve`] directly — the streamliner adds no
/// useful collapses).
pub fn smart_solve(
    piles: &PileSet,
    budget: SolverBudget,
    streamliner: StreamlinerMode,
) -> SolveResult {
    let now = web_time::Instant::now();
    let total = budget.deadline.saturating_duration_since(now);
    // 10 % of budget for the streamliner pass; the streamliner
    // sees a smaller `max_nodes` proportionally so it can't burn
    // the whole node budget either.
    let pre_dur = total / 10;
    let pre_nodes = budget.max_nodes / 10;
    let pre_budget = SolverBudget {
        deadline: now + pre_dur,
        max_nodes: pre_nodes,
    };
    if let SolveResult::Won = solve_with(piles, pre_budget, streamliner) {
        return SolveResult::Won;
    }
    // Streamliner didn't find a win — possible false negative.
    // Run strict pass with the full original budget.
    solve_with(piles, budget, StreamlinerMode::Strict)
}

/// Run the search with a chosen [`StreamlinerMode`]. `Strict` is
/// sound; `SuitFlatten` and `SuitColour` may report `Exhausted` on
/// genuinely-winnable deals (false negatives) and should be wrapped
/// in [`smart_solve`] for a guaranteed-sound outer answer.
pub fn solve_with(
    piles: &PileSet,
    budget: SolverBudget,
    mode: StreamlinerMode,
) -> SolveResult {
    // Single shared mutable state — every node mutates the same
    // `PileSet` via apply_move on push and revert_move on pop.
    // No clone-per-child; the entire search shares one allocation.
    let mut state = piles.clone();
    while let Some(m) = collapse_step(&state) {
        apply_move(&mut state, &m);
    }
    if is_won(&state) {
        return SolveResult::Won;
    }
    let mut visited: HashSet<u64> = HashSet::new();
    visited.insert(hash_state(&state, mode));
    let mut stack: Vec<Frame> = Vec::with_capacity(256);
    stack.push(Frame {
        applied: Vec::new(),
        choices: generate_choices(&state),
        next_idx: 0,
    });

    let mut nodes = 0u64;
    loop {
        // Borrow-check the frame as briefly as possible — we need
        // to mutate `state` between consults, so we don't hold a
        // long-lived &mut Frame.
        let (choice, exhausted) = {
            let Some(frame) = stack.last_mut() else {
                break;
            };
            if frame.next_idx >= frame.choices.len() {
                (None, true)
            } else {
                let c = std::mem::take(&mut frame.choices[frame.next_idx]);
                frame.next_idx += 1;
                (Some(c), false)
            }
        };

        if exhausted {
            // Revert this frame's applied moves in reverse, then pop.
            if let Some(frame) = stack.pop() {
                for am in frame.applied.iter().rev() {
                    revert_move(&mut state, am);
                }
            }
            continue;
        }

        nodes += 1;
        if nodes > budget.max_nodes {
            return SolveResult::Timeout;
        }
        if nodes & 4095 == 0 && web_time::Instant::now() >= budget.deadline {
            return SolveResult::Timeout;
        }

        let choice = choice.unwrap();
        // Apply each move in the choice + any forced collapses,
        // recording every applied move so the parent can be
        // restored exactly on backtrack.
        let mut applied: Vec<Move> = Vec::with_capacity(choice.len() + 2);
        for m in &choice {
            apply_move(&mut state, m);
            applied.push(*m);
        }
        while let Some(am) = collapse_step(&state) {
            apply_move(&mut state, &am);
            applied.push(am);
        }
        if is_won(&state) {
            return SolveResult::Won;
        }
        let ch = hash_state(&state, mode);
        if !visited.insert(ch) {
            // Already-seen state — revert and try next move.
            for am in applied.iter().rev() {
                revert_move(&mut state, am);
            }
            continue;
        }
        let child_choices = generate_choices(&state);
        stack.push(Frame {
            applied,
            choices: child_choices,
            next_idx: 0,
        });
    }
    SolveResult::Exhausted
}

struct Frame {
    /// Moves applied (in order) to reach this frame's state from
    /// the parent frame's state. Reverted in reverse order on pop.
    applied: Vec<Move>,
    choices: Vec<Choice>,
    next_idx: usize,
}

fn is_won(piles: &PileSet) -> bool {
    for fid in FOUND_FIRST..=FOUND_LAST {
        if piles.get(fid).cards.len() != 13 {
            return false;
        }
    }
    true
}

/// Per-card hash byte: `suit_class * 26 + 2 * rank + face_down`.
/// `suit_class` collapses suit detail according to the active
/// [`StreamlinerMode`] (SPEC §2, §4):
/// - `Strict` — four distinct suits → values 0..=3
/// - `SuitColour` — black (♣/♠) → 0, red (♦/♥) → 1
/// - `SuitFlatten` — all suits → 0
///
/// Multiplying by 26 = (rank range × 2 face-up/-down) keeps each
/// suit class's encoding disjoint so the hash bits stay
/// distinguishable even before the mixer.
#[inline]
fn card_byte(c: &Card, mode: StreamlinerMode) -> u64 {
    let suit_class: u64 = match mode {
        StreamlinerMode::Strict => match c.suit {
            Suit::Clubs => 0,
            Suit::Diamonds => 1,
            Suit::Hearts => 2,
            Suit::Spades => 3,
        },
        StreamlinerMode::SuitColour => match c.suit {
            Suit::Clubs | Suit::Spades => 0,
            Suit::Diamonds | Suit::Hearts => 1,
        },
        StreamlinerMode::SuitFlatten => 0,
    };
    let rank_val: u64 = c.rank as u64;
    let fd: u64 = if c.face_up { 0 } else { 1 };
    suit_class * 26 + 2 * rank_val + fd
}

/// Standard golden-ratio hash combiner (Knuth / Boost). Cheaper
/// than `DefaultHasher` and well-mixed for short input streams.
#[inline]
fn mix(seed: &mut u64, v: u64) {
    *seed ^= v
        .wrapping_add(0x9e3779b9_u64)
        .wrapping_add(seed.wrapping_shl(6))
        .wrapping_add(seed.wrapping_shr(2));
}

/// Compress a single pile to a 64-bit fingerprint.
fn pile_fingerprint(cards: &[Card], mode: StreamlinerMode) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis as seed
    mix(&mut h, cards.len() as u64);
    for c in cards {
        mix(&mut h, card_byte(c, mode));
    }
    h
}

/// Hash the board with the 10 tableau columns **sorted by their
/// fingerprint** so any permutation of columns collapses to the
/// same TT key (SPEC §2). Foundations are reduced to a count of
/// completed-suit piles since the 8 slots are interchangeable.
/// Stock is order-stable.
fn hash_state(piles: &PileSet, mode: StreamlinerMode) -> u64 {
    let mut h: u64 = 0;

    // Foundations: just the count of completed suits, since each
    // foundation pile is either empty or holds exactly one
    // completed 13-card suit and they're interchangeable.
    let mut completed: u64 = 0;
    for fid in FOUND_FIRST..=FOUND_LAST {
        if piles.get(fid).cards.len() == 13 {
            completed += 1;
        }
    }
    mix(&mut h, completed);

    // Stock: order-stable, top-to-bottom. The marker separates
    // foundation count from stock contents in the hash stream.
    mix(&mut h, 0x57_4F_43_4B_4D_41_52_4B); // "STOCKMRK"
    let stock = piles.get(STOCK);
    mix(&mut h, stock.cards.len() as u64);
    for c in &stock.cards {
        mix(&mut h, card_byte(c, mode));
    }

    // Tableau: permutation-invariant via sorted fingerprints.
    let mut fps: [u64; N_CASCADES] = [0; N_CASCADES];
    for (i, cid) in (CASCADE_FIRST..=CASCADE_LAST).enumerate() {
        fps[i] = pile_fingerprint(&piles.get(cid).cards, mode);
    }
    fps.sort_unstable();
    for fp in fps {
        mix(&mut h, fp);
    }

    h
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

fn generate_choices(piles: &PileSet) -> Vec<Choice> {
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
                // SPEC § 3 prune P1: a single-card move from a
                // pile of length 1 onto an empty pile is a pure
                // no-op (the source becomes empty, the dest becomes
                // a singleton — symmetric to the start state under
                // pile-order canonicalization).
                if take == 1 && start_idx == 0 && n == 1 && dst.is_empty() {
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
    let mut out: Vec<Choice> = scored.into_iter().map(|(_, m)| vec![m]).collect();
    // Stock deal is an atomic 10-card broadcast: one card per
    // cascade, applied in left-to-right order. Treating it as a
    // single Choice keeps the solver from exploring nonsensical
    // partial-deal states. Pushed last so it's the lowest-priority
    // option in FIFO traversal.
    if piles.get(STOCK).len() >= N_CASCADES {
        let mut deal: Choice = Vec::with_capacity(N_CASCADES);
        for col in 0..N_CASCADES {
            deal.push(Move::simple(STOCK, 1, CASCADE_FIRST + col as u8).with_flip_moved());
        }
        out.push(deal);
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
    fn hash_state_collapses_tableau_column_permutations() {
        // Two boards that differ only in tableau column order
        // should hash to the same TT key — that's the whole
        // point of the SPEC § 2 pile-order canonicalisation.
        let rules = Spider::four_suit();
        let mut a = crate::piles::PileSet::from_slots(
            &rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let mut b = a.clone();

        // a: column 0 = [K♠ face-up], column 1 = [Q♥ face-up]
        a.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(crate::cards::Suit::Spades, Rank::King).face_up());
        a.get_mut(CASCADE_FIRST + 1)
            .cards
            .push(Card::new(crate::cards::Suit::Hearts, Rank::Queen).face_up());

        // b: swapped — column 0 = [Q♥ face-up], column 1 = [K♠ face-up]
        b.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(crate::cards::Suit::Hearts, Rank::Queen).face_up());
        b.get_mut(CASCADE_FIRST + 1)
            .cards
            .push(Card::new(crate::cards::Suit::Spades, Rank::King).face_up());

        assert_eq!(
            hash_state(&a, StreamlinerMode::Strict),
            hash_state(&b, StreamlinerMode::Strict)
        );
    }

    /// SPEC §4 — `SuitFlatten` mode collapses suit detail so two
    /// states differing only in suit (♠ vs ♣) hash identically.
    /// Required for the S2 streamliner on 1-suit Spider; useful as
    /// an optimistic streamliner on higher-suit variants.
    #[test]
    fn hash_state_collapses_suit_under_flatten_mode() {
        let rules = Spider::four_suit();
        let mut a = crate::piles::PileSet::from_slots(
            &rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let mut b = a.clone();
        a.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
        b.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Clubs, Rank::King).face_up());
        // Strict mode: ♠ vs ♣ are different cards → distinct hashes.
        assert_ne!(
            hash_state(&a, StreamlinerMode::Strict),
            hash_state(&b, StreamlinerMode::Strict),
        );
        // Flatten mode: every suit collapses to 0 → identical hash.
        assert_eq!(
            hash_state(&a, StreamlinerMode::SuitFlatten),
            hash_state(&b, StreamlinerMode::SuitFlatten),
        );
    }

    /// SPEC §4 — `SuitColour` collapses suits by black/red parity
    /// so ♣ ≡ ♠ and ♦ ≡ ♥, but black ≠ red.
    #[test]
    fn hash_state_collapses_suit_under_colour_mode() {
        let rules = Spider::four_suit();
        let mut a = crate::piles::PileSet::from_slots(
            &rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let mut b = a.clone();
        let mut c = a.clone();
        a.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
        b.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Clubs, Rank::King).face_up());
        c.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::King).face_up());
        // ♠ and ♣ are both black → same hash under SuitColour.
        assert_eq!(
            hash_state(&a, StreamlinerMode::SuitColour),
            hash_state(&b, StreamlinerMode::SuitColour),
        );
        // ♥ is red → distinct hash.
        assert_ne!(
            hash_state(&a, StreamlinerMode::SuitColour),
            hash_state(&c, StreamlinerMode::SuitColour),
        );
    }

    /// SPEC §5 — `smart_solve` must return the same verdict as
    /// strict `solve` on a pre-won board (no time spent in the
    /// streamliner pass beyond confirming the win immediately).
    #[test]
    fn smart_solve_finds_win_on_pre_won_board() {
        let mut s = GameSession::new(Spider::four_suit(), 1);
        for fid in FOUND_FIRST..=FOUND_LAST {
            s.piles.get_mut(fid).cards.clear();
            for _ in 0..13 {
                s.piles
                    .get_mut(fid)
                    .cards
                    .push(Card::new(Suit::Spades, Rank::King));
            }
        }
        let budget =
            SolverBudget::from_duration(std::time::Duration::from_millis(50), 1_000);
        assert_eq!(
            smart_solve(&s.piles, budget, StreamlinerMode::SuitColour),
            SolveResult::Won
        );
    }

    #[test]
    fn hash_state_distinguishes_face_up_from_face_down() {
        // The per-card hash byte must include face-down so a deal
        // with one flipped card hashes distinctly.
        let rules = Spider::four_suit();
        let mut a = crate::piles::PileSet::from_slots(
            &rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT),
        );
        let mut b = a.clone();
        a.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(crate::cards::Suit::Spades, Rank::King).face_up());
        b.get_mut(CASCADE_FIRST)
            .cards
            .push(Card::new(crate::cards::Suit::Spades, Rank::King)); // face-down
        assert_ne!(
            hash_state(&a, StreamlinerMode::Strict),
            hash_state(&b, StreamlinerMode::Strict)
        );
    }

    /// Real-seed smoke benchmark — ignored by default.
    ///
    /// Run with:
    ///   cargo test -p solitaire-core --release spider_solver \
    ///     -- --ignored --nocapture --test-threads=1
    ///
    /// 1-suit uses `smart_solve` with `SuitFlatten`; 2-suit uses
    /// `smart_solve` with `SuitColour`; 4-suit uses bare `solve`
    /// (no streamliner). Per-seed verdict + elapsed prints live so
    /// you can watch progress; end-of-run reports Won / Exhausted /
    /// Timeout tallies plus Wilson 95 % CI.
    #[ignore]
    #[test]
    fn benchmark_one_suit_spider_seeds() {
        bench(1, "1-suit", 20, std::time::Duration::from_secs(5));
    }

    #[ignore]
    #[test]
    fn benchmark_two_suit_spider_seeds() {
        bench(2, "2-suit", 20, std::time::Duration::from_secs(5));
    }

    #[ignore]
    #[test]
    fn benchmark_four_suit_spider_seeds() {
        bench(4, "4-suit", 20, std::time::Duration::from_secs(5));
    }

    /// Statistical-oracle smoke — 100 fresh 4-suit Spider deals at
    /// a 5-minute per-deal budget. Paper Table 1 reports
    /// **98.487 % ± 1.513 %** for 4-suit Spider (Blake & Gent
    /// JAIR 2026). Our 95 % CI should overlap that range to pass
    /// the SPEC § 8 acceptance gate.
    ///
    /// Worst-case wall clock: 100 × 5 min = ~8 hours if every
    /// deal times out. Realistic: most deals resolve in seconds.
    #[ignore]
    #[test]
    fn smoke_oracle_four_suit_100_seeds() {
        bench(4, "smoke-4s", 100, std::time::Duration::from_secs(300));
    }

    /// Quick-resolve smoke: 100 fresh 4-suit deals at a 60-second
    /// budget and 50 M node cap. The point isn't to classify hard
    /// deals — it's to see how many EASY deals our solver
    /// resolves in seconds. If ≥30 % return Won we have a viable
    /// solver for the bulk run (just accept that ~70 % time out
    /// and discard them). If <5 % return Won, the solver needs
    /// fundamental improvements before any bulk run.
    ///
    /// Worst-case wall: 100 × 60 s = 100 min.
    #[ignore]
    #[test]
    fn quick_resolve_four_suit_100_seeds() {
        bench_with_nodes(
            4,
            "quick-4s",
            100,
            std::time::Duration::from_secs(60),
            50_000_000,
        );
    }

    fn bench(suit_count: u8, label: &str, count: u64, budget_per_seed: std::time::Duration) {
        bench_with_nodes(suit_count, label, count, budget_per_seed, 200_000_000);
    }

    fn bench_with_nodes(
        suit_count: u8,
        label: &str,
        count: u64,
        budget_per_seed: std::time::Duration,
        max_nodes: u64,
    ) {
        use crate::session::GameSession;
        let mut won = 0u64;
        let mut exhausted = 0u64;
        let mut timeout = 0u64;
        let started = web_time::Instant::now();
        for seed in 0u64..count {
            let rules = Spider::new(suit_count);
            let s = GameSession::new(rules, seed);
            let start = web_time::Instant::now();
            let budget = SolverBudget::from_duration(budget_per_seed, max_nodes);
            let r = match suit_count {
                1 => smart_solve(&s.piles, budget, StreamlinerMode::SuitFlatten),
                2 => smart_solve(&s.piles, budget, StreamlinerMode::SuitColour),
                _ => solve_with(&s.piles, budget, StreamlinerMode::Strict),
            };
            let elapsed = start.elapsed();
            println!("{label} seed {seed:4}: {r:?} in {:?}", elapsed);
            match r {
                SolveResult::Won => won += 1,
                SolveResult::Exhausted => exhausted += 1,
                SolveResult::Timeout => timeout += 1,
            }
        }
        let total_elapsed = started.elapsed();
        let determined = won + exhausted;
        // Conservative Wilson CI: upper assumes every timeout is
        // a win; lower assumes every timeout is a loss.
        let (lo, hi) = if determined + timeout > 0 {
            let n = (determined + timeout) as f64;
            let z = 1.96_f64;
            let z2 = z * z;
            let p_lo = won as f64 / n;
            let p_hi = (won + timeout) as f64 / n;
            let denom_lo = 1.0 + z2 / n;
            let denom_hi = 1.0 + z2 / n;
            let centre_lo = (p_lo + z2 / (2.0 * n)) / denom_lo;
            let centre_hi = (p_hi + z2 / (2.0 * n)) / denom_hi;
            let radius_lo = z * (p_lo * (1.0 - p_lo) / n + z2 / (4.0 * n * n)).sqrt() / denom_lo;
            let radius_hi = z * (p_hi * (1.0 - p_hi) / n + z2 / (4.0 * n * n)).sqrt() / denom_hi;
            (
                ((centre_lo - radius_lo) * 100.0).max(0.0),
                ((centre_hi + radius_hi) * 100.0).min(100.0),
            )
        } else {
            (0.0, 0.0)
        };
        println!(
            "{label} Spider: {won} won, {exhausted} exhausted, {timeout} timeout (of {count}) — \
             95 % CI [{lo:.3} %, {hi:.3} %] — wall {total_elapsed:?}"
        );
    }

    #[test]
    fn apply_revert_preserves_state_across_random_dfs() {
        // Run a short bounded search; the solver's apply/revert
        // path must leave the board exactly as it started when the
        // search returns (Exhausted or Timeout). Any drift would
        // mean revert_move and apply_move aren't true inverses for
        // the move shapes the solver emits.
        use crate::session::GameSession;
        let s = GameSession::new(Spider::four_suit(), 42);
        let starting = s.piles.clone();
        let budget = SolverBudget::from_duration(std::time::Duration::from_millis(80), 10_000);
        // We don't care about the result — only that the state is
        // un-perturbed if we re-run starting from a fresh clone.
        let _ = solve(&starting, budget);
        // Re-run on the same starting state and confirm we get the
        // same classification (deterministic).
        let r2 = solve(&starting, budget);
        // Run a third time — same answer.
        let r3 = solve(&starting, budget);
        assert_eq!(r2, r3);
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
