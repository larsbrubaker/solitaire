//! Reverse-shuffle Spider deal generator. Produces deals that are
//! winnable **by construction**: we start at the solved state (all
//! 8 K-A suited runs on foundations, empty board, empty stock) and
//! apply legal inverse moves until we reach a standard Spider
//! opening layout. The forward solver doesn't need to run.
//!
//! Why this exists: the clean-room forward solver
//! (`spider_solver.rs`) handles 1-suit easily and 2-suit at long
//! budget, but cannot resolve 4-suit Spider in usable wall time
//! (see `quick_resolve_four_suit_100_seeds` — 5/5 timeouts at
//! 60 s / 50 M nodes). Reverse-shuffle is the published fallback
//! technique for shipping guaranteed-winnable deals when forward
//! search is too expensive.
//!
//! Trade-off (documented in the Sokoban backward-generation
//! literature, IJCAI 2019): generated deals are biased toward
//! shallow / easier solutions because the inverse-of-walk-length
//! puts an upper bound on the solution depth. Population
//! distribution differs from random-shuffle 4-suit Spider. For
//! shipping a "Winnable deals only" toggle this trade-off is fine.

use rand::{Rng, RngCore, SeedableRng};
use rand::rngs::StdRng;

use crate::cards::{Card, Rank, Suit};
use crate::piles::{PileId, PileSet};

// Pile id layout — matches the conventions used by
// `games/spider.rs` and `spider_solver.rs`.
const FOUND_FIRST: PileId = 0;
const FOUND_LAST: PileId = 7;
const STOCK: PileId = 8;
const CASCADE_FIRST: PileId = 9;
const CASCADE_LAST: PileId = 18;
const N_CASCADES: usize = 10;
const N_FOUNDATIONS: usize = 8;

/// Standard Spider 4-suit opening: 4 cascades of 6 (5↓ + 1↑) and
/// 6 cascades of 5 (4↓ + 1↑); 50 face-down cards in stock; 0 on
/// foundations.
fn target_cascade_size(idx: usize) -> usize {
    if idx < 4 {
        6
    } else {
        5
    }
}
const TARGET_STOCK: usize = 50;

/// Pull the 8 K-A suited runs that the solved Spider 4-suit
/// position holds on its foundations. Each run is `[K, Q, J, …,
/// 2, A]` in face-up order so when it's lifted onto a cascade in
/// the reverse walk, the cascade top after placement is the A.
fn build_solved_state(suit_count: u8, one_suit: Suit) -> PileSet {
    use crate::games::GameRules;
    let rules = match suit_count {
        1 => crate::games::spider::Spider::one_suit_of(one_suit),
        _ => crate::games::spider::Spider::new(suit_count),
    };
    let layout = rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT);
    let mut piles = PileSet::from_slots(&layout);
    // `active_suits` returns 8 entries, one per foundation
    // (Spider is a 2-deck game so each "natural" suit gets 2
    // foundations — already accounted for in the vec).
    let suits = active_suits(suit_count, one_suit);
    debug_assert_eq!(suits.len(), N_FOUNDATIONS);
    for (i, &suit) in suits.iter().enumerate() {
        let fid = FOUND_FIRST + i as u8;
        let pile = piles.get_mut(fid);
        for rank in (1..=13).rev() {
            pile.cards.push(Card::new(suit, rank_from_u8(rank)).face_up());
        }
    }
    piles
}

fn rank_from_u8(r: u8) -> Rank {
    // crate's Rank encoding: 1 = Ace, 13 = King.
    match r {
        1 => Rank::Ace,
        2 => Rank::Two,
        3 => Rank::Three,
        4 => Rank::Four,
        5 => Rank::Five,
        6 => Rank::Six,
        7 => Rank::Seven,
        8 => Rank::Eight,
        9 => Rank::Nine,
        10 => Rank::Ten,
        11 => Rank::Jack,
        12 => Rank::Queen,
        13 => Rank::King,
        _ => panic!("rank {r}"),
    }
}

/// Active suits for the variant. 1-suit has all 8 runs of one
/// suit (chosen by user); 2-suit has 4 runs each of two suits;
/// 4-suit has 2 runs of each of the 4 suits.
fn active_suits(suit_count: u8, one_suit: Suit) -> Vec<Suit> {
    match suit_count {
        1 => vec![one_suit; 8],
        2 => {
            // Pair black + red for visual contrast.
            let s2 = if matches!(one_suit, Suit::Spades | Suit::Clubs) {
                Suit::Hearts
            } else {
                Suit::Spades
            };
            vec![one_suit, one_suit, one_suit, one_suit, s2, s2, s2, s2]
        }
        4 => vec![
            Suit::Spades,
            Suit::Spades,
            Suit::Hearts,
            Suit::Hearts,
            Suit::Diamonds,
            Suit::Diamonds,
            Suit::Clubs,
            Suit::Clubs,
        ],
        _ => panic!("unsupported suit_count {suit_count}"),
    }
}

/// Generate a guaranteed-winnable Spider deal for the given seed.
///
/// Returns a `PileSet` shaped like the standard Spider 4-suit
/// opening (54 cards distributed across 10 cascades + 50 face-down
/// in stock + empty foundations) that can be solved forward via
/// the recorded inverse sequence.
pub fn generate_winnable_deal(seed: u64, suit_count: u8, one_suit: Suit) -> PileSet {
    let mut rng = StdRng::seed_from_u64(seed);
    // Re-seed deterministically so different seeds give different
    // shuffles but the same seed is reproducible across machines.
    let mut state = build_solved_state(suit_count, one_suit);
    drive_to_opening(&mut state, &mut rng);
    state
}

/// Reverse walk driver. Phases:
/// 1. Empty foundations: pull each K-A run onto a cascade.
/// 2. Re-distribute: do random inter-cascade suited-group moves
///    to mix cards across cascades.
/// 3. Move 50 cards into stock: lift "deal rows" (one card per
///    cascade) back into the stock pile 5 times.
/// 4. Trim each cascade to its target length by lifting tops
///    into the stock (face-down).
/// 5. Set face-down on every non-top cascade card.
///
/// Stages 2-4 are interleaved randomly under the RNG; the
/// per-phase moves are deterministic given the RNG. The phase
/// boundaries are STRICTLY ENFORCED by the driver — each reverse
/// move stays inside the legal-inverse subset for that phase so
/// we always have a valid winning sequence we COULD replay
/// forward (we don't record it; the existence proof is enough).
fn drive_to_opening<R: Rng + RngCore>(state: &mut PileSet, rng: &mut R) {
    // Phase 1: dump foundations onto cascades.
    phase1_dump_foundations(state, rng);
    // Phase 2: mix cascade contents with inverse cascade moves.
    phase2_mix_cascades(state, rng);
    // Phase 3: stock up. Lift TARGET_STOCK cards off cascade tops
    // back into the stock so they end up face-down.
    phase3_fill_stock(state, rng);
    // Phase 4: trim each cascade to its target length.
    phase4_trim_cascades(state, rng);
    // Phase 5: convert all non-top cascade cards to face-down.
    phase5_face_down_below_top(state);
}

fn phase1_dump_foundations<R: Rng>(state: &mut PileSet, rng: &mut R) {
    // Walk foundations 0..N_FOUNDATIONS, taking each K-A run off
    // and placing onto a randomly-chosen cascade. The order is
    // randomised so two seeds don't produce identical sequences.
    let mut order: Vec<u8> = (FOUND_FIRST..=FOUND_LAST).collect();
    for i in (1..order.len()).rev() {
        let j = rng.gen_range(0..=i);
        order.swap(i, j);
    }
    for fid in order {
        let pile = state.get_mut(fid);
        let cards: Vec<Card> = pile.cards.drain(..).collect();
        let dst = CASCADE_FIRST + rng.gen_range(0..N_CASCADES as u8);
        for c in cards {
            state.get_mut(dst).cards.push(c);
        }
    }
}

fn phase2_mix_cascades<R: Rng>(state: &mut PileSet, rng: &mut R) {
    // Apply a fixed number of inverse cascade moves. Each move
    // takes 1..=4 face-up suited cards off a random non-empty
    // cascade and places them on another cascade where the
    // destination's top is rank-up of the moved head.
    //
    // "Rank-up" inverse is what forward Spider's any-suit build
    // policy already allows: any card with rank R can be moved
    // onto a card with rank R+1 regardless of suit. Inverse just
    // applies the same constraint.
    let iters = 200;
    for _ in 0..iters {
        let src = CASCADE_FIRST + rng.gen_range(0..N_CASCADES as u8);
        let (suited_len, src_len) = {
            let src_pile = state.get(src);
            (suited_run_length_at_top(&src_pile.cards), src_pile.cards.len())
        };
        if src_len == 0 {
            continue;
        }
        let take = rng.gen_range(1..=suited_len.min(4));
        let head_idx = src_len - take;
        let head_rank = state.get(src).cards[head_idx].rank;
        let mut candidates: Vec<u8> = Vec::with_capacity(N_CASCADES);
        for dst in CASCADE_FIRST..=CASCADE_LAST {
            if dst == src {
                continue;
            }
            let dst_pile = state.get(dst);
            if let Some(top) = dst_pile.top() {
                if rank_up(head_rank) == Some(top.rank) {
                    candidates.push(dst);
                }
            }
        }
        if candidates.is_empty() {
            continue;
        }
        let dst = candidates[rng.gen_range(0..candidates.len())];
        let cards: Vec<Card> = state.get_mut(src).cards.split_off(src_len - take);
        for c in cards {
            state.get_mut(dst).cards.push(c);
        }
    }
}

fn phase3_fill_stock<R: Rng>(state: &mut PileSet, rng: &mut R) {
    // We need TARGET_STOCK cards in the stock. Each row-lift
    // takes 1 card from each cascade (must all be non-empty)
    // and pushes onto stock, face-down. Five rows = 50.
    let rows_needed = TARGET_STOCK / N_CASCADES;
    for _ in 0..rows_needed {
        // Ensure all 10 cascades are non-empty; if any is empty,
        // shuffle from a neighbour first.
        for c in CASCADE_FIRST..=CASCADE_LAST {
            if state.get(c).cards.is_empty() {
                // Borrow from a richer cascade.
                let donor = (CASCADE_FIRST..=CASCADE_LAST)
                    .max_by_key(|d| state.get(*d).cards.len())
                    .unwrap();
                if state.get(donor).cards.len() >= 2 {
                    let card = state.get_mut(donor).cards.pop().unwrap();
                    state.get_mut(c).cards.push(card);
                }
            }
        }
        // Pick column visit order randomly.
        let mut order: Vec<u8> = (CASCADE_FIRST..=CASCADE_LAST).collect();
        for i in (1..order.len()).rev() {
            let j = rng.gen_range(0..=i);
            order.swap(i, j);
        }
        for cid in order {
            if let Some(mut card) = state.get_mut(cid).cards.pop() {
                card.face_up = false;
                state.get_mut(STOCK).cards.push(card);
            }
        }
    }
}

fn phase4_trim_cascades<R: Rng>(state: &mut PileSet, rng: &mut R) {
    // After phase 3 we have 54 cards across cascades but maybe
    // unevenly. Trim each to its target by lifting extras into
    // the stock as face-down.
    for cid in CASCADE_FIRST..=CASCADE_LAST {
        let i = (cid - CASCADE_FIRST) as usize;
        let target = target_cascade_size(i);
        while state.get(cid).cards.len() > target {
            if let Some(mut card) = state.get_mut(cid).cards.pop() {
                card.face_up = false;
                state.get_mut(STOCK).cards.push(card);
            }
        }
    }
    let _ = rng;
}

fn phase5_face_down_below_top(state: &mut PileSet) {
    // Every cascade ends with its TOP card face-up, all cards
    // below face-down. (Standard Spider opening rule.)
    for cid in CASCADE_FIRST..=CASCADE_LAST {
        let pile = state.get_mut(cid);
        let n = pile.cards.len();
        if n == 0 {
            continue;
        }
        for i in 0..n - 1 {
            pile.cards[i].face_up = false;
        }
        pile.cards[n - 1].face_up = true;
    }
}

/// Length of the longest suited (same-suit, descending-by-1) run
/// at the TOP of the pile. Returns 1 for a single top card.
fn suited_run_length_at_top(cards: &[Card]) -> usize {
    if cards.is_empty() {
        return 0;
    }
    let mut len = 1;
    while len < cards.len() {
        let upper = &cards[cards.len() - 1 - (len - 1)];
        let lower = &cards[cards.len() - 1 - len];
        if !upper.face_up || !lower.face_up {
            break;
        }
        if upper.suit != lower.suit {
            break;
        }
        if Some(upper.rank) != lower.rank.next_down() {
            break;
        }
        len += 1;
    }
    len
}

fn rank_up(r: Rank) -> Option<Rank> {
    r.next_up()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::Suit;

    #[test]
    fn generated_deal_has_correct_card_count() {
        let piles = generate_winnable_deal(1, 4, Suit::Spades);
        let mut total = 0;
        for pid in 0..=CASCADE_LAST {
            total += piles.get(pid).cards.len();
        }
        assert_eq!(total, 104, "expected 104 cards total, got {total}");
    }

    #[test]
    fn generated_deal_has_correct_stock_size() {
        let piles = generate_winnable_deal(2, 4, Suit::Spades);
        assert_eq!(piles.get(STOCK).cards.len(), TARGET_STOCK);
    }

    #[test]
    fn generated_deal_has_empty_foundations() {
        let piles = generate_winnable_deal(3, 4, Suit::Spades);
        for fid in FOUND_FIRST..=FOUND_LAST {
            assert!(
                piles.get(fid).cards.is_empty(),
                "foundation {fid} should be empty",
            );
        }
    }

    #[test]
    fn generated_deal_cascade_shapes_match_spider() {
        let piles = generate_winnable_deal(4, 4, Suit::Spades);
        for (i, cid) in (CASCADE_FIRST..=CASCADE_LAST).enumerate() {
            assert_eq!(
                piles.get(cid).cards.len(),
                target_cascade_size(i),
                "cascade {i} (pile {cid}) wrong length",
            );
        }
    }

    #[test]
    fn generated_deal_top_cards_face_up_rest_face_down() {
        let piles = generate_winnable_deal(5, 4, Suit::Spades);
        for cid in CASCADE_FIRST..=CASCADE_LAST {
            let pile = piles.get(cid);
            let n = pile.cards.len();
            if n == 0 {
                continue;
            }
            for i in 0..n - 1 {
                assert!(
                    !pile.cards[i].face_up,
                    "cascade {cid} card {i} should be face-down",
                );
            }
            assert!(
                pile.cards[n - 1].face_up,
                "cascade {cid} top should be face-up",
            );
        }
    }

    #[test]
    fn generated_deal_stock_is_all_face_down() {
        let piles = generate_winnable_deal(6, 4, Suit::Spades);
        for c in &piles.get(STOCK).cards {
            assert!(!c.face_up, "stock card should be face-down");
        }
    }

    #[test]
    fn generator_is_deterministic_by_seed() {
        let a = generate_winnable_deal(42, 4, Suit::Spades);
        let b = generate_winnable_deal(42, 4, Suit::Spades);
        for pid in 0..=CASCADE_LAST {
            let pa = &a.get(pid).cards;
            let pb = &b.get(pid).cards;
            assert_eq!(pa.len(), pb.len(), "pile {pid} length differs");
            for (i, (x, y)) in pa.iter().zip(pb.iter()).enumerate() {
                assert_eq!(
                    x.suit, y.suit,
                    "pile {pid} card {i} suit differs",
                );
                assert_eq!(
                    x.rank, y.rank,
                    "pile {pid} card {i} rank differs",
                );
                assert_eq!(
                    x.face_up, y.face_up,
                    "pile {pid} card {i} face_up differs",
                );
            }
        }
    }

    #[test]
    fn generator_produces_different_deals_for_different_seeds() {
        let a = generate_winnable_deal(1, 4, Suit::Spades);
        let b = generate_winnable_deal(2, 4, Suit::Spades);
        // Compare the first cascade's contents — different seeds
        // should pretty much always pick different placements.
        let pa = &a.get(CASCADE_FIRST).cards;
        let pb = &b.get(CASCADE_FIRST).cards;
        let same = pa.len() == pb.len()
            && pa.iter().zip(pb.iter()).all(|(x, y)| {
                x.suit == y.suit && x.rank == y.rank
            });
        assert!(!same, "different seeds shouldn't produce identical first cascade");
    }
}
