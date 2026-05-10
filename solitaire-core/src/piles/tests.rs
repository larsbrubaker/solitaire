use super::*;
use crate::cards::{Card, Rank, Suit};
use crate::consts::{CARD_H, CARD_W};

fn k_of_spades() -> Card {
    Card::new(Suit::Spades, Rank::King).face_up()
}

#[test]
fn empty_pile_hit_returns_empty_slot() {
    let p = Pile::new(0, PileKind::Tableau, PileLayout::Stacked, 100.0, 100.0);
    assert_eq!(
        p.hit_test(110.0, 110.0),
        Some(HitResult::EmptySlot { pile: 0 })
    );
    // Outside the slot.
    assert_eq!(p.hit_test(50.0, 50.0), None);
}

#[test]
fn stacked_pile_hit_returns_top_card() {
    let mut p = Pile::new(0, PileKind::Foundation, PileLayout::Stacked, 100.0, 100.0);
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
    let mut p = Pile::new(0, PileKind::Tableau, PileLayout::FannedDown, 100.0, 500.0);
    // Three face-up cards fanned down.
    for _ in 0..3 {
        p.cards.push(k_of_spades());
    }
    // Card 2 (top) is at y = 500 + (-28) + (-28) = 444. Click in its center.
    let (_, y2, _, _) = p.card_rect(2);
    let hit = p.hit_test(100.0 + 10.0, y2 + 60.0);
    assert_eq!(
        hit,
        Some(HitResult::Card {
            pile: 0,
            card_idx: 2
        })
    );

    // Click on the visible strip of card 0 (only the very top is visible
    // above card 1). Card 0 occupies y in [500, 500+126]; card 1 starts at
    // y=472 so card 0's visible strip is y in [500+126-?, 500+126].
    // card 0's top edge: y = 500 + 126 = 626. Click at y = 622, just above
    // card 1's top edge of 472+126=598. So card 0 wins at y=622.
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
    // Waste pile with 5 cards and a 3-card fan offset of 20px each.
    let mut p = Pile::new(0, PileKind::Waste, PileLayout::Stacked, 100.0, 200.0);
    for _ in 0..5 {
        p.cards.push(k_of_spades());
    }
    p.fan_top_n = 3;
    p.fan_dx = 20.0;

    // idx 0 and 1 are below the fan group: stay at origin_x.
    let (x0, _) = p.position_for(0);
    let (x1, _) = p.position_for(1);
    assert_eq!(x0, 100.0);
    assert_eq!(x1, 100.0);

    // idx 2,3,4 are the fan group: 0, 20, 40 px past origin_x.
    let (x2, _) = p.position_for(2);
    let (x3, _) = p.position_for(3);
    let (x4, _) = p.position_for(4);
    assert_eq!(x2, 100.0);
    assert_eq!(x3, 120.0);
    assert_eq!(x4, 140.0);
}

#[test]
fn waste_fan_only_topmost_is_hittable() {
    // Even though three cards visually overlap in the fan, only the
    // topmost (rightmost) is interactable — Microsoft Solitaire UX.
    let mut p = Pile::new(0, PileKind::Waste, PileLayout::Stacked, 100.0, 200.0);
    for _ in 0..3 {
        p.cards.push(k_of_spades());
    }
    p.fan_top_n = 3;
    p.fan_dx = 20.0;

    // Click on the visible left edge of the bottom card in the fan
    // (idx 0, x in [100, 120]). Without the fan rule, this would hit idx 0;
    // with it, the click should miss because only the top card is hittable.
    let hit_left = p.hit_test(105.0, 200.0 + CARD_H / 2.0);
    assert_eq!(hit_left, None);

    // Click squarely on the topmost (idx 2) card at its right edge.
    let hit_top = p.hit_test(100.0 + 40.0 + CARD_W / 2.0, 200.0 + CARD_H / 2.0);
    assert_eq!(
        hit_top,
        Some(HitResult::Card {
            pile: 0,
            card_idx: 2,
        })
    );
}
