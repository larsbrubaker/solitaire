use super::*;
use crate::cards::{Card, Rank, Suit};
use crate::piles::{PileKind, PileLayout, PileSet};

fn two_pile_set() -> PileSet {
    use crate::piles::PileSlot;
    PileSet::from_slots(&[
        PileSlot {
            id: 0,
            kind: PileKind::Tableau,
            layout: PileLayout::FannedDown,
            origin_x: 0.0,
            origin_y: 0.0,
        },
        PileSlot {
            id: 1,
            kind: PileKind::Foundation,
            layout: PileLayout::Stacked,
            origin_x: 200.0,
            origin_y: 0.0,
        },
    ])
}

#[test]
fn apply_then_revert_round_trips() {
    let mut piles = two_pile_set();
    piles
        .get_mut(0)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
    piles
        .get_mut(0)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Two).face_up());
    let before = piles.clone();

    let m = Move::simple(0, 1, 1);
    apply_move(&mut piles, &m);
    assert_eq!(piles.get(0).len(), 1);
    assert_eq!(piles.get(1).len(), 1);
    assert_eq!(piles.get(1).top().unwrap().rank, Rank::Two);

    revert_move(&mut piles, &m);
    assert_eq!(piles.get(0).cards, before.get(0).cards);
    assert_eq!(piles.get(1).cards, before.get(1).cards);
}

#[test]
fn flip_source_after_reveals_facedown() {
    let mut piles = two_pile_set();
    piles
        .get_mut(0)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Ace)); // face-down
    piles
        .get_mut(0)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Two).face_up());

    let m = Move::simple(0, 1, 1).with_flip_source();
    apply_move(&mut piles, &m);
    assert_eq!(piles.get(0).len(), 1);
    assert!(piles.get(0).top().unwrap().face_up);

    revert_move(&mut piles, &m);
    assert_eq!(piles.get(0).len(), 2);
    assert!(!piles.get(0).cards[0].face_up);
}

#[test]
fn flip_moved_toggles_face_state() {
    let mut piles = two_pile_set();
    piles
        .get_mut(0)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Ace)); // face-down

    let m = Move::simple(0, 1, 1).with_flip_moved();
    apply_move(&mut piles, &m);
    assert!(piles.get(1).top().unwrap().face_up);

    revert_move(&mut piles, &m);
    assert!(!piles.get(0).top().unwrap().face_up);
}
