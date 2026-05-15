//! Regression tests that lock the (seed → deal) mapping for every
//! variant. The bundled winnable-seeds lists are produced offline by
//! running the Solvitaire validator against the deals our engine
//! produces; if the engine's shuffle ever drifts (RNG swap, deck
//! re-order, deal-loop tweak) the bundled seeds stop matching the
//! validated deals.
//!
//! These tests hash the post-deal `PileSet` (every card slot, in
//! order, including face-up flags) and compare against a frozen
//! value. A failure here is a heads-up that the bundled lists need
//! regenerating.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::games::freecell::FreeCell;
use crate::games::klondike::Klondike;
use crate::games::spider::Spider;
use crate::piles::PileSet;
use crate::session::GameSession;

fn deal_hash(piles: &PileSet) -> u64 {
    let mut h = DefaultHasher::new();
    for pile in piles.iter() {
        pile.id.hash(&mut h);
        pile.cards.len().hash(&mut h);
        pile.cards.hash(&mut h);
    }
    h.finish()
}

#[test]
fn klondike_seed_42_deal_is_stable() {
    let s = GameSession::new(Klondike::with_draw_count(1), 42);
    assert_eq!(
        deal_hash(&s.piles),
        9_163_243_335_169_517_362,
        "Klondike seed=42 deal hash drifted; regenerate the bundled \
         klondike_winnable_seeds.bin before shipping."
    );
}

#[test]
fn freecell_random_seed_99_deal_is_stable() {
    // FreeCell without winnable-only uses the regular `StdRng` path.
    let s = GameSession::new(FreeCell::new(), 99);
    assert_eq!(
        deal_hash(&s.piles),
        1_276_558_292_901_738_601,
        "FreeCell seed=99 random-mode deal hash drifted."
    );
}

#[test]
fn freecell_ms_game_1_deal_is_stable() {
    // Microsoft FreeCell Game #1 is the canonical regression deal —
    // every clone of the algorithm prints the same opening row.
    let s = GameSession::new(FreeCell::with_ms_game_number(1), 1);
    assert_eq!(
        deal_hash(&s.piles),
        4_384_499_236_828_697_820,
        "Microsoft FreeCell Game #1 deal hash drifted; the LCG \
         shuffle has diverged from Jim Horne's reference algorithm."
    );
}

#[test]
fn freecell_ms_game_617_deal_is_stable() {
    // Game #617 is one of the most-cited tutorial deals in classic
    // FreeCell write-ups; locking it gives us a second independent
    // check on the LCG path.
    let s = GameSession::new(FreeCell::with_ms_game_number(617), 617);
    assert_eq!(
        deal_hash(&s.piles),
        3_109_503_398_623_144_311,
        "Microsoft FreeCell Game #617 deal hash drifted."
    );
}

#[test]
fn spider_four_suit_seed_7_deal_is_stable() {
    let s = GameSession::new(Spider::four_suit(), 7);
    assert_eq!(
        deal_hash(&s.piles),
        14_465_209_736_344_745_356,
        "Spider 4-suit seed=7 deal hash drifted; regenerate the \
         bundled spider_winnable_seeds.bin before shipping."
    );
}
