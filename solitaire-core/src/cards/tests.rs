use super::*;

#[test]
fn standard_deck_has_52_unique_cards() {
    let deck = standard_deck();
    assert_eq!(deck.len(), 52);
    let mut seen = std::collections::HashSet::new();
    for c in &deck {
        assert!(
            seen.insert((c.suit, c.rank)),
            "duplicate {:?}-{:?}",
            c.suit,
            c.rank
        );
    }
}

#[test]
fn shuffle_is_deterministic_given_seed() {
    let a = shuffled_seeded(42);
    let b = shuffled_seeded(42);
    assert_eq!(a, b);
    let c = shuffled_seeded(43);
    assert_ne!(a, c);
}

#[test]
fn rank_next_up_down_round_trips() {
    for r in Rank::ALL {
        if let Some(up) = r.next_up() {
            assert_eq!(up.next_down(), Some(r));
        }
    }
    assert_eq!(Rank::Ace.next_down(), None);
    assert_eq!(Rank::King.next_up(), None);
}

#[test]
fn suit_color_is_red_for_hearts_and_diamonds() {
    assert_eq!(Suit::Hearts.color(), CardColor::Red);
    assert_eq!(Suit::Diamonds.color(), CardColor::Red);
    assert_eq!(Suit::Spades.color(), CardColor::Black);
    assert_eq!(Suit::Clubs.color(), CardColor::Black);
}
