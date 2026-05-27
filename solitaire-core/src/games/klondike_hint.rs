//! Klondike Hint ranker. Heuristic scorer that picks the best
//! visible move for the current board; used by the Hint button and
//! the `H` hotkey. Lives in a sibling file so `klondike.rs` stays
//! under the 800-line cap.

use crate::cards::Rank;
use crate::piles::PileSet;

use super::hint::Hint;
use super::klondike::{
    alt_color_descending, is_tableau, is_valid_run, same_suit_ascending, Klondike, FOUND_FIRST,
    FOUND_LAST, STOCK, TABLEAU_FIRST, TABLEAU_LAST, WASTE,
};

/// Pick the highest-value Klondike move for the current board.
///
/// Ranking (higher is better, lexicographic):
/// 1. Move that completes a card to its foundation slot (always a
///    safe play that strictly advances the game).
/// 2. Move that exposes a face-down tableau card.
/// 3. Move that places a King onto an empty tableau column (frees
///    a slot for further manoeuvring).
/// 4. Suited / alt-color tableau build that extends a run by at
///    least one card.
/// 5. Waste → tableau placement (gets a waste card into play).
/// 6. Stock click — only when nothing else looks productive.
///
/// Returns `None` only when there's literally no legal move and no
/// stock to deal — i.e. the game is over (won or stuck).
pub fn best_klondike_hint(rules: &Klondike, piles: &PileSet) -> Option<Hint> {
    let mut best: Option<(i64, Hint)> = None;
    let consider = |score: i64, hint: Hint, best: &mut Option<(i64, Hint)>| {
        let beats = match best {
            None => true,
            Some((s, _)) => score > *s,
        };
        if beats {
            *best = Some((score, hint));
        }
    };

    // 1. Tableau top → foundation, or waste top → foundation.
    for src in (TABLEAU_FIRST..=TABLEAU_LAST).chain(std::iter::once(WASTE)) {
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
            if !ok {
                continue;
            }
            // Score 10000 baseline; +1000 if move exposes a
            // face-down underneath (sending a tableau top home AND
            // revealing a card is a double-win).
            let exposes = is_tableau(src)
                && pile.cards.len() > 1
                && !pile.cards[pile.cards.len() - 2].face_up;
            let mut score = 10_000i64;
            if exposes {
                score += 1_000;
            }
            // Stable tie-break by ascending source pile id so the
            // ranker doesn't oscillate between equally-good moves.
            score -= src as i64;
            consider(
                score,
                Hint::Move {
                    from: src,
                    start_idx: pile.cards.len() - 1,
                    take: 1,
                    to: fid,
                },
                &mut best,
            );
        }
    }

    // 2 & 3 & 4. Tableau face-up runs → tableau.
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
                // Skip moving a King to an empty column when the
                // King is already on top of an empty column —
                // pointless cycle.
                if dst.is_empty() && head.rank == Rank::King && start_idx == 0 {
                    continue;
                }
                let mut score = 0i64;
                if exposes {
                    score += 5_000;
                }
                if dst.is_empty() && head.rank == Rank::King {
                    score += 1_500;
                }
                // Encourage longer-tail moves (more cards into a
                // tidy run).
                score += take as i64 * 50;
                score -= src_id as i64;
                score -= dst_id as i64;
                let m = Hint::Move {
                    from: src_id,
                    start_idx,
                    take: take as u8,
                    to: dst_id,
                };
                let _ = exposes;
                consider(score, m, &mut best);
            }
        }
    }

    // 5. Waste → tableau.
    {
        let waste = piles.get(WASTE);
        if let Some(top) = waste.top() {
            for dst_id in TABLEAU_FIRST..=TABLEAU_LAST {
                let dst = piles.get(dst_id);
                let legal = match dst.top() {
                    None => top.rank == Rank::King,
                    Some(dt) => dt.face_up && alt_color_descending(dt, top),
                };
                if !legal {
                    continue;
                }
                let mut score = 800i64;
                if dst.is_empty() && top.rank == Rank::King {
                    score += 500;
                }
                score -= dst_id as i64;
                consider(
                    score,
                    Hint::Move {
                        from: WASTE,
                        start_idx: waste.cards.len() - 1,
                        take: 1,
                        to: dst_id,
                    },
                    &mut best,
                );
            }
        }
    }

    if let Some((_, hint)) = best {
        return Some(hint);
    }

    // 6. Fallback: stock click. If stock has cards, drawing is
    // legal; if waste has cards, recycling onto the stock is the
    // engine-permitted equivalent. Either way the hint just points
    // the player at the stock pile.
    let stock_or_waste_nonempty = !piles.get(STOCK).is_empty() || !piles.get(WASTE).is_empty();
    if stock_or_waste_nonempty {
        let _ = rules;
        return Some(Hint::StockDeal { stock: STOCK });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Card, Suit};
    use crate::games::GameRules;
    use crate::piles::PileSet;

    fn empty_klondike() -> (Klondike, PileSet) {
        let rules = Klondike::new();
        let piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
        (rules, piles)
    }

    #[test]
    fn hint_prefers_foundation_over_tableau_build() {
        let (rules, mut piles) = empty_klondike();
        // Ace on T1 — legal to foundation. Also a 10♣ on T2 and
        // J♥ on T3 so a tableau build is available too.
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
        piles
            .get_mut(TABLEAU_FIRST + 1)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
        piles
            .get_mut(TABLEAU_FIRST + 2)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Jack).face_up());
        let h = best_klondike_hint(&rules, &piles).expect("hint exists");
        match h {
            Hint::Move { from, to, take, .. } => {
                assert_eq!(from, TABLEAU_FIRST);
                assert!((FOUND_FIRST..=FOUND_LAST).contains(&to));
                assert_eq!(take, 1);
            }
            Hint::StockDeal { .. } => panic!("expected foundation move, got stock"),
        }
    }

    #[test]
    fn hint_prefers_exposing_move_over_plain_build() {
        let (rules, mut piles) = empty_klondike();
        // T1: [face-down X, 10♣ face-up] — moving 10♣ exposes X.
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Diamonds, Rank::Two));
        piles
            .get_mut(TABLEAU_FIRST)
            .cards
            .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
        // T2: [J♥ face-up] — landing spot for 10♣.
        piles
            .get_mut(TABLEAU_FIRST + 1)
            .cards
            .push(Card::new(Suit::Hearts, Rank::Jack).face_up());
        // T3: [J♦ face-up] — alternate, non-exposing target.
        piles
            .get_mut(TABLEAU_FIRST + 2)
            .cards
            .push(Card::new(Suit::Diamonds, Rank::Jack).face_up());
        // T4: [9♠ face-up] — sits on top of nothing, no exposure.
        piles
            .get_mut(TABLEAU_FIRST + 3)
            .cards
            .push(Card::new(Suit::Spades, Rank::Nine).face_up());
        let h = best_klondike_hint(&rules, &piles).expect("hint exists");
        match h {
            Hint::Move { from, .. } => assert_eq!(from, TABLEAU_FIRST),
            Hint::StockDeal { .. } => panic!("expected tableau move"),
        }
    }

    #[test]
    fn hint_falls_back_to_stock_when_no_moves() {
        let (rules, mut piles) = empty_klondike();
        // Just a face-down card in stock; no other piles populated.
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two));
        let h = best_klondike_hint(&rules, &piles).expect("hint exists");
        match h {
            Hint::StockDeal { stock } => assert_eq!(stock, STOCK),
            Hint::Move { .. } => panic!("expected stock fallback"),
        }
    }

    #[test]
    fn hint_returns_none_on_dead_board() {
        let (rules, piles) = empty_klondike();
        assert!(best_klondike_hint(&rules, &piles).is_none());
    }
}
