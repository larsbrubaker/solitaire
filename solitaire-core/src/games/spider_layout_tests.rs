//! Fan-scale layout tests for Spider — split from `spider_tests.rs`
//! to keep that file under the 800-line cap.

use super::*;

#[test]
fn portrait_rect_scales_cascade_fans() {
    let rules = Spider::four_suit();
    let rect = Rect::new(0.0, 0.0, 390.0, 800.0);
    let slots = rules.pile_layout(rect);
    let scale = slots[CASCADE_FIRST as usize].fan_scale;
    assert!(scale > 1.0, "portrait rect must stretch cascade fans");
    assert!(scale <= 2.0);
    for i in 0..N_CASCADES as u8 {
        assert_eq!(slots[(CASCADE_FIRST + i) as usize].fan_scale, scale);
    }
    // Only cascades stretch — stock and foundations keep 1.0.
    assert_eq!(slots[STOCK as usize].fan_scale, 1.0);
    for i in 0..8u8 {
        assert_eq!(slots[(FOUND_FIRST + i) as usize].fan_scale, 1.0);
    }
    // Worst-case cascade (5 face-down + K→A run of 13 face-up) must
    // still fit above the playfield bottom at this scale.
    let mut pile = crate::piles::Pile::from_slot(&slots[CASCADE_FIRST as usize]);
    for _ in 0..5 {
        pile.cards.push(Card::new(Suit::Spades, Rank::King));
    }
    for _ in 0..13 {
        pile.cards
            .push(Card::new(Suit::Spades, Rank::King).face_up());
    }
    let (_, y_bottom) = pile.position_for(pile.cards.len() - 1);
    assert!(
        y_bottom >= rect.y,
        "worst-case cascade bottom {y_bottom} overflows the playfield"
    );
}

#[test]
fn height_bound_rect_keeps_default_fan_scale() {
    let rules = Spider::four_suit();
    let slots = rules.pile_layout(Rect::new(0.0, 0.0, 1600.0, 700.0));
    for i in 0..N_CASCADES as u8 {
        let s = slots[(CASCADE_FIRST + i) as usize].fan_scale;
        assert!(
            (s - 1.0).abs() < 1e-9,
            "height-bound fit must not stretch fans, got {s}"
        );
    }
}
