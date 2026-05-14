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
fn stock_click_blocked_when_any_cascade_empty() {
    let rules = Spider::four_suit();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    for _ in 0..50 {
        piles
            .get_mut(STOCK)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two));
    }
    // Cascade 0 left empty → click should yield no moves.
    for cid in CASCADE_FIRST + 1..=CASCADE_LAST {
        piles
            .get_mut(cid)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
    }
    assert!(rules.on_pile_click(&piles, STOCK).is_empty());
}
