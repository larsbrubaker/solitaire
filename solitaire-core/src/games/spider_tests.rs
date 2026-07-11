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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
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
fn hint_prefers_move_that_exposes_face_down() {
    // Cascade 0 has a single face-up 6 (leftmost; no facedown to expose).
    // Cascade 1 has [face-down 5♥, face-up 6♠] (exposing move).
    // Cascade 2 has a 7♣ — both 6s can land on it.
    // Hint must pick the exposing move, beating the leftmost tiebreaker.
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(CASCADE_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Six).face_up());
    piles
        .get_mut(CASCADE_FIRST + 1)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Five));
    piles
        .get_mut(CASCADE_FIRST + 1)
        .cards
        .push(Card::new(Suit::Spades, Rank::Six).face_up());
    piles
        .get_mut(CASCADE_FIRST + 2)
        .cards
        .push(Card::new(Suit::Clubs, Rank::Seven).face_up());

    let hint = best_spider_hint(&piles).expect("a legal move exists");
    assert_eq!(
        hint,
        SpiderHint::Move {
            from: CASCADE_FIRST + 1,
            start_idx: 1,
            take: 1,
            to: CASCADE_FIRST + 2,
        }
    );
}

#[test]
fn hint_prefers_extending_suited_run() {
    // Two legal 6→7 moves; one creates a suited run (6♠ on 7♠), the
    // other does not (6♥ on 7♠). Hint picks the suited one even
    // though the alternative is leftmost.
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(CASCADE_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Six).face_up());
    piles
        .get_mut(CASCADE_FIRST + 1)
        .cards
        .push(Card::new(Suit::Spades, Rank::Six).face_up());
    piles
        .get_mut(CASCADE_FIRST + 2)
        .cards
        .push(Card::new(Suit::Spades, Rank::Seven).face_up());

    let hint = best_spider_hint(&piles).expect("a legal move exists");
    assert_eq!(
        hint,
        SpiderHint::Move {
            from: CASCADE_FIRST + 1,
            start_idx: 0,
            take: 1,
            to: CASCADE_FIRST + 2,
        }
    );
}

#[test]
fn hint_falls_back_to_stock_deal_when_no_tableau_move() {
    // Every cascade tops out at a King; Kings can only move to empty
    // cascades and none are empty. Stock has enough cards to deal.
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    for cid in CASCADE_FIRST..=CASCADE_LAST {
        piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
    }
    for _ in 0..N_CASCADES {
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two));
    }
    let hint = best_spider_hint(&piles).expect("stock-deal fallback");
    assert_eq!(hint, SpiderHint::StockDeal { stock: STOCK });
}

#[test]
fn hint_skips_sterile_duplicate_parent_shuffle() {
    // Regression (user report): one cascade tops out at Q-J (suited
    // spades), another at Q-J-10-9-...-A (suited spades). Moving the
    // 10-down-to-A onto the lone J just relocates the suited tail
    // under an identical J — no facedown exposed, no run completed,
    // no real progress. Hint must NOT recommend this move.
    let rules = Spider::one_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    let short = CASCADE_FIRST;
    for r in [Rank::Queen, Rank::Jack] {
        piles
            .get_mut(short)
            .cards
            .push(Card::new(Suit::Spades, r).face_up());
    }
    let long = CASCADE_FIRST + 1;
    for r in [
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
            .get_mut(long)
            .cards
            .push(Card::new(Suit::Spades, r).face_up());
    }
    // Keep the remaining cascades non-empty + stock primed so the
    // legal-fallback paths exist; the test only asserts that the
    // sterile shuffle isn't picked.
    for cid in (CASCADE_FIRST + 2)..=CASCADE_LAST {
        piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
    }
    for _ in 0..N_CASCADES {
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two));
    }

    let hint = best_spider_hint(&piles).expect("stock-deal fallback at minimum");
    if let SpiderHint::Move { from, to, .. } = hint {
        assert!(
            !(from == long && to == short),
            "Hint must not pick the J\u{2192}J duplicate-parent shuffle"
        );
    }
}

#[test]
fn hint_returns_none_when_only_sterile_relocations_remain() {
    // User report: two columns and a stray card, every legal move is
    // either a duplicate-parent shuffle (Q-J on one cascade, Q-J-10-…-A
    // on another) or a wholesale relocation of a suited run to an
    // empty cascade. None of those advance the game, and the stock is
    // empty, so the hint must return None (the UI then shows the
    // "No moves" toast instead of a misleading highlight).
    let rules = Spider::one_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    // Short column: Q-J.
    let short = CASCADE_FIRST;
    for r in [Rank::Queen, Rank::Jack] {
        piles
            .get_mut(short)
            .cards
            .push(Card::new(Suit::Spades, r).face_up());
    }
    // Long column: Q down through A, all suited.
    let long = CASCADE_FIRST + 1;
    for r in [
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
            .get_mut(long)
            .cards
            .push(Card::new(Suit::Spades, r).face_up());
    }
    // A single stray 2 (mirrors the screenshot's middle column).
    piles
        .get_mut(CASCADE_FIRST + 2)
        .cards
        .push(Card::new(Suit::Spades, Rank::Two).face_up());
    // Other cascades empty, stock empty — no stock deal is legal.
    assert!(best_spider_hint(&piles).is_none());
}

#[test]
fn hint_returns_none_when_no_tableau_and_stock_illegal() {
    // Same locked-Kings board, but stock has zero cards so the deal
    // is illegal too. Hint must report no move at all.
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    for cid in CASCADE_FIRST..=CASCADE_LAST {
        piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
    }
    assert!(best_spider_hint(&piles).is_none());
}

#[test]
fn stock_click_deals_even_with_empty_cascade() {
    // We deviate from classic Spider's "all cascades must be non-empty"
    // rule: a board that's run out of productive moves but still has
    // rows in the stock should be able to deal. Empty cascades just
    // get one face-up card each from the deal.
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    for _ in 0..50 {
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two));
    }
    // Cascade 0 left empty; other cascades populated.
    for cid in CASCADE_FIRST + 1..=CASCADE_LAST {
        piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
    }
    let moves = rules.on_pile_click(&piles, STOCK);
    assert_eq!(moves.len(), N_CASCADES, "deal still fires");
    // The first dealt move targets the empty cascade — that's the
    // whole point of relaxing the rule.
    assert_eq!(moves[0].to, CASCADE_FIRST);
}

#[test]
fn stock_click_still_blocked_when_stock_too_short() {
    // The relaxation only covers empty cascades; if the stock can't
    // cover all ten cascades we still refuse to deal a partial row.
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    for _ in 0..(N_CASCADES - 1) {
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two));
    }
    for cid in CASCADE_FIRST..=CASCADE_LAST {
        piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
    }
    assert!(rules.on_pile_click(&piles, STOCK).is_empty());
}

#[test]
fn wide_rect_picks_side_column_layout() {
    let rules = Spider::four_suit();
    let rect = Rect::new(0.0, 0.0, 1600.0, 700.0);
    let slots = rules.pile_layout(rect);
    let top = crate::games::fit_cards(rect, 10, 10.0, 12.0, 4.0);
    let side = crate::games::fit_cards(rect, 12, 10.0, 12.0, 3.2);
    assert!(
        side.card_h > top.card_h,
        "side candidate must win on a wide rect"
    );
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(eq(slots[STOCK as usize].card_h, side.card_h));
    // Left column: 8 foundations stacked with overlapping origins,
    // spread across the full column height. RE-PINNED: the step is no
    // longer a fixed 0.15·card_h — it spreads to fill `rect.height`,
    // clamped to [floor, card_h + col_gap]. Here the column has ample
    // room, so the step exceeds the 0.15 floor.
    let min_step = side.card_h * 0.15;
    let cap = side.card_h + 10.0;
    let step = ((rect.height - side.card_h) / 7.0).clamp(min_step, cap);
    assert!(step > min_step, "wide column must spread past the floor");
    for i in 0..8u8 {
        let f = &slots[(FOUND_FIRST + i) as usize];
        assert!(eq(f.origin_x, side.left));
        assert!(eq(f.origin_y, side.top_row_origin_y - i as f64 * step));
    }
    // The 8 slots span no more than the full column height (extent =
    // card_h + 7·step ≤ rect.height when the step isn't floor-clamped).
    let extent = side.card_h + 7.0 * step;
    assert!(
        extent <= rect.height + 1e-9,
        "stack overflows column height"
    );
    // Cascades: columns 1..=10, full playfield height.
    for i in 0..10u8 {
        let t = &slots[(CASCADE_FIRST + i) as usize];
        assert!(eq(t.origin_x, side.left + (1 + i) as f64 * side.col_pitch));
        assert!(eq(t.origin_y, side.top_row_origin_y));
    }
    // Right column: stock, top-aligned.
    assert!(eq(
        slots[STOCK as usize].origin_x,
        side.left + 11.0 * side.col_pitch
    ));
    assert!(eq(slots[STOCK as usize].origin_y, side.top_row_origin_y));
}

#[test]
fn tall_rect_keeps_top_row_layout() {
    let rules = Spider::four_suit();
    let slots = rules.pile_layout(Rect::new(0.0, 0.0, 390.0, 800.0));
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-6;
    // Pin the historical top-row layout: width-bound cards of
    // (390 - 9*10) / 10 = 30 wide, aspect 1.4 → 42 tall.
    let card_w = 30.0;
    let card_h = card_w * crate::games::CARD_ASPECT;
    let col_pitch = card_w + 10.0;
    assert!(eq(slots[STOCK as usize].card_h, card_h));
    // Foundations across columns 0..=7 in the top row; stock at
    // column 9 (column 8 is the visual gap); cascades one row-pitch
    // below at column 0.
    assert!(eq(slots[FOUND_FIRST as usize].origin_x, 0.0));
    assert!(eq(slots[FOUND_FIRST as usize].origin_y, 800.0 - card_h));
    assert!(eq(
        slots[(FOUND_FIRST + 7) as usize].origin_x,
        7.0 * col_pitch
    ));
    assert!(eq(slots[STOCK as usize].origin_x, 9.0 * col_pitch));
    assert!(eq(slots[STOCK as usize].origin_y, 800.0 - card_h));
    assert!(eq(slots[CASCADE_FIRST as usize].origin_x, 0.0));
    assert!(eq(
        slots[CASCADE_FIRST as usize].origin_y,
        800.0 - card_h - (card_h + 12.0)
    ));
}
