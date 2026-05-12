use super::*;
use crate::cards::{Card, Rank, Suit};

/// Reference card dimensions for tests that predate the per-game
/// dynamic sizing. Numerically identical to the historical
/// `consts::CARD_W` / `CARD_H` so existing test math doesn't shift.
const CARD_W: f64 = 90.0;
const CARD_H: f64 = 126.0;

fn k_of_spades() -> Card {
    Card::new(Suit::Spades, Rank::King).face_up()
}

/// Build a test pile with the legacy 90×126 card size at the given
/// origin / kind / layout.
fn pile(id: PileId, kind: PileKind, layout: PileLayout, ox: f64, oy: f64) -> Pile {
    Pile::from_slot(&PileSlot::new(id, kind, layout, ox, oy, CARD_W, CARD_H))
}

#[test]
fn empty_pile_hit_returns_empty_slot() {
    let p = pile(0, PileKind::Tableau, PileLayout::Stacked, 100.0, 100.0);
    assert_eq!(
        p.hit_test(110.0, 110.0),
        Some(HitResult::EmptySlot { pile: 0 })
    );
    assert_eq!(p.hit_test(50.0, 50.0), None);
}

#[test]
fn stacked_pile_hit_returns_top_card() {
    let mut p = pile(0, PileKind::Foundation, PileLayout::Stacked, 100.0, 100.0);
    p.cards.push(k_of_spades());
    p.cards.push(k_of_spades());
    let hit = p.hit_test(100.0 + CARD_W / 2.0, 100.0 + CARD_H / 2.0);
    assert_eq!(
        hit,
        Some(HitResult::Card {
            pile: 0,
            card_idx: 1
        })
    );
}

#[test]
fn fanned_down_pile_hits_correct_card() {
    let mut p = pile(0, PileKind::Tableau, PileLayout::FannedDown, 100.0, 500.0);
    for _ in 0..3 {
        p.cards.push(k_of_spades());
    }
    let (_, y2, _, _) = p.card_rect(2);
    let hit = p.hit_test(100.0 + 10.0, y2 + 60.0);
    assert_eq!(
        hit,
        Some(HitResult::Card {
            pile: 0,
            card_idx: 2
        })
    );

    // Click on the visible strip of card 0 (the very top of the fan).
    let hit_top0 = p.hit_test(100.0 + 10.0, 620.0);
    assert_eq!(
        hit_top0,
        Some(HitResult::Card {
            pile: 0,
            card_idx: 0
        })
    );
}

#[test]
fn waste_fan_offsets_top_n_cards_horizontally() {
    let mut p = pile(0, PileKind::Waste, PileLayout::Stacked, 100.0, 200.0);
    for _ in 0..5 {
        p.cards.push(k_of_spades());
    }
    p.fan_top_n = 3;
    p.fan_dx = 20.0;

    let (x0, _) = p.position_for(0);
    let (x1, _) = p.position_for(1);
    assert_eq!(x0, 100.0);
    assert_eq!(x1, 100.0);

    let (x2, _) = p.position_for(2);
    let (x3, _) = p.position_for(3);
    let (x4, _) = p.position_for(4);
    assert_eq!(x2, 100.0);
    assert_eq!(x3, 120.0);
    assert_eq!(x4, 140.0);
}

#[test]
fn waste_fan_only_topmost_is_hittable() {
    let mut p = pile(0, PileKind::Waste, PileLayout::Stacked, 100.0, 200.0);
    for _ in 0..3 {
        p.cards.push(k_of_spades());
    }
    p.fan_top_n = 3;
    p.fan_dx = 20.0;

    let hit_left = p.hit_test(105.0, 200.0 + CARD_H / 2.0);
    assert_eq!(hit_left, None);

    let hit_top = p.hit_test(100.0 + 40.0 + CARD_W / 2.0, 200.0 + CARD_H / 2.0);
    assert_eq!(
        hit_top,
        Some(HitResult::Card {
            pile: 0,
            card_idx: 2,
        })
    );
}
