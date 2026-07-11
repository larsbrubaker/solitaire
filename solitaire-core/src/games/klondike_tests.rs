use super::*;
use crate::cards::Suit;
use crate::session::{apply_move, GameSession};

#[test]
fn deal_distributes_28_cards_to_tableau_and_24_to_stock() {
    let s = GameSession::new(Klondike::new(), 7);
    let total_tableau: usize = (TABLEAU_FIRST..=TABLEAU_LAST)
        .map(|id| s.piles.get(id).len())
        .sum();
    assert_eq!(total_tableau, 28);
    assert_eq!(s.piles.get(STOCK).len(), 24);
    assert_eq!(s.piles.get(WASTE).len(), 0);
    // Each tableau column has its top card face-up.
    for id in TABLEAU_FIRST..=TABLEAU_LAST {
        let p = s.piles.get(id);
        assert!(p.top().unwrap().face_up);
    }
    // Force-pop unused-warning suppression when iterating.
    let _ = s.piles.get(STOCK);
}

#[test]
fn ace_to_empty_foundation_is_legal() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(TABLEAU_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
    let m = Move::simple(TABLEAU_FIRST, 1, FOUND_FIRST);
    assert!(rules.legal_move(&piles, &m));
}

#[test]
fn two_to_empty_foundation_is_illegal() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(TABLEAU_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Two).face_up());
    let m = Move::simple(TABLEAU_FIRST, 1, FOUND_FIRST);
    assert!(!rules.legal_move(&piles, &m));
}

#[test]
fn alternating_descending_run_to_tableau_is_legal() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    // src tableau has [10♣ face-up], dest tableau has [J♥ face-up].
    piles
        .get_mut(TABLEAU_FIRST)
        .cards
        .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
    piles
        .get_mut(TABLEAU_FIRST + 1)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Jack).face_up());
    let m = Move::simple(TABLEAU_FIRST, 1, TABLEAU_FIRST + 1);
    assert!(rules.legal_move(&piles, &m));
}

#[test]
fn same_color_to_tableau_is_illegal() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(TABLEAU_FIRST)
        .cards
        .push(Card::new(Suit::Spades, Rank::Ten).face_up());
    piles
        .get_mut(TABLEAU_FIRST + 1)
        .cards
        .push(Card::new(Suit::Clubs, Rank::Jack).face_up());
    let m = Move::simple(TABLEAU_FIRST, 1, TABLEAU_FIRST + 1);
    assert!(!rules.legal_move(&piles, &m));
}

#[test]
fn king_to_empty_tableau_is_legal() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(WASTE)
        .cards
        .push(Card::new(Suit::Spades, Rank::King).face_up());
    let m = Move::simple(WASTE, 1, TABLEAU_FIRST);
    assert!(rules.legal_move(&piles, &m));
}

#[test]
fn single_click_top_card_prefers_foundation() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(TABLEAU_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
    piles
        .get_mut(TABLEAU_FIRST + 1)
        .cards
        .push(Card::new(Suit::Spades, Rank::Two).face_up());

    let m = rules
        .single_click_move(&piles, TABLEAU_FIRST, 0)
        .expect("ace can move to foundation");
    assert_eq!(m.from, TABLEAU_FIRST);
    assert_eq!(m.to, FOUND_FIRST);
    assert_eq!(m.take, 1);
}

#[test]
fn single_click_run_moves_to_leftmost_legal_tableau() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    let src = TABLEAU_FIRST + 4;
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Nine).face_up());

    let left_dst = TABLEAU_FIRST + 1;
    let right_dst = TABLEAU_FIRST + 3;
    for dst in [left_dst, right_dst] {
        piles
            .get_mut(dst)
            .cards
            .push(Card::new(Suit::Diamonds, Rank::Jack).face_up());
    }

    let m = rules
        .single_click_move(&piles, src, 0)
        .expect("10-9 run can move onto either jack");
    assert_eq!(m.from, src);
    assert_eq!(m.to, left_dst);
    assert_eq!(m.take, 2);
}

#[test]
fn stock_click_when_nonempty_dispenses_one_to_waste() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(STOCK)
        .cards
        .push(Card::new(Suit::Spades, Rank::Ace));
    let moves = rules.on_pile_click(&piles, STOCK);
    assert_eq!(moves.len(), 1);
    let m = moves[0];
    assert!(rules.legal_move(&piles, &m));
    apply_move(&mut piles, &m);
    assert_eq!(piles.get(WASTE).len(), 1);
    assert!(piles.get(WASTE).top().unwrap().face_up);
}

// Rects at which each of the three candidate arrangements wins
// (derived from the 7/10/9-column budgets 3.6/2.8/2.8):
//   TopRow      — a tall portrait rect (width-bound top row wins).
//   SideColumns — a very wide, short rect (height binds; the 10- and
//                 9-col sides tie and the tie-break keeps 2x2).
//   SideStacked — a moderately wide rect (width binds; 9 cols beats
//                 10 with a strictly larger card).
const TOPROW_RECT: Rect = Rect::new(0.0, 0.0, 390.0, 800.0);
const SIDECOL_RECT: Rect = Rect::new(0.0, 0.0, 2000.0, 500.0);
const STACKED_RECT: Rect = Rect::new(0.0, 0.0, 1600.0, 700.0);

#[test]
fn side_columns_2x2_wins_on_wide_short_rect() {
    let rules = Klondike::with_draw_count(3);
    let slots = rules.pile_layout(SIDECOL_RECT);
    let side = crate::games::fit_cards(SIDECOL_RECT, 10, 12.0, 12.0, 2.8);
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(eq(slots[STOCK as usize].card_h, side.card_h));
    // Left column: stock on top, waste one row below with a downward
    // 3-draw fan.
    assert!(eq(slots[STOCK as usize].origin_x, side.left));
    assert!(eq(slots[STOCK as usize].origin_y, side.top_row_origin_y));
    let w = &slots[WASTE as usize];
    assert!(eq(w.origin_x, side.left));
    assert!(eq(w.origin_y, side.top_row_origin_y - side.row_pitch));
    assert_eq!(w.fan_top_n, 3);
    assert!(eq(w.fan_dy, -side.card_h * 0.27));
    // Tableau columns 1..=7, full height.
    for i in 0..7u8 {
        let t = &slots[(TABLEAU_FIRST + i) as usize];
        assert!(eq(t.origin_x, side.left + (1 + i) as f64 * side.col_pitch));
        assert!(eq(t.origin_y, side.top_row_origin_y));
    }
    // Foundations form a 2x2 grid across columns 8 AND 9 — the
    // distinguishing mark of this arrangement.
    for i in 0..4u8 {
        let f = &slots[(FOUND_FIRST + i) as usize];
        assert!(eq(
            f.origin_x,
            side.left + (8 + i % 2) as f64 * side.col_pitch
        ));
        assert!(eq(
            f.origin_y,
            side.top_row_origin_y - (i / 2) as f64 * side.row_pitch
        ));
    }
    // found[1] sits one column right of found[0] (2x2, not stacked).
    assert!(slots[(FOUND_FIRST + 1) as usize].origin_x > slots[FOUND_FIRST as usize].origin_x);
}

#[test]
fn side_stacked_wins_on_moderately_wide_rect() {
    let rules = Klondike::with_draw_count(3);
    let slots = rules.pile_layout(STACKED_RECT);
    let fit = crate::games::fit_cards(STACKED_RECT, 9, 12.0, 12.0, 2.8);
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-9;
    // 9-col stacked fit beats both the 7-col top row and 10-col 2x2.
    let top = crate::games::fit_cards(STACKED_RECT, 7, 12.0, 12.0, 3.6);
    let side2x2 = crate::games::fit_cards(STACKED_RECT, 10, 12.0, 12.0, 2.8);
    assert!(fit.card_h > top.card_h && fit.card_h > side2x2.card_h);
    assert!(eq(slots[STOCK as usize].card_h, fit.card_h));
    // Left column: stock on top, waste one row below.
    assert!(eq(slots[STOCK as usize].origin_x, fit.left));
    assert!(eq(slots[WASTE as usize].origin_x, fit.left));
    // All four foundations share column 8, stepping DOWN. RE-PINNED:
    // the step now spreads across the full column height (clamped to
    // [0.28·card_h floor, card_h + col_gap]) instead of a fixed
    // 0.28·card_h. This tall column spreads past the floor.
    let min_step = fit.card_h * STACKED_FOUNDATION_STEP;
    let cap = fit.card_h + 12.0;
    let step = ((STACKED_RECT.height - fit.card_h) / 3.0).clamp(min_step, cap);
    assert!(step > min_step, "tall column must spread past the floor");
    for i in 0..4u8 {
        let f = &slots[(FOUND_FIRST + i) as usize];
        assert!(eq(f.origin_x, fit.left + 8.0 * fit.col_pitch));
        assert!(eq(f.origin_y, fit.top_row_origin_y - i as f64 * step));
    }
    // Tableau columns 1..=7, full height.
    for i in 0..7u8 {
        let t = &slots[(TABLEAU_FIRST + i) as usize];
        assert!(eq(t.origin_x, fit.left + (1 + i) as f64 * fit.col_pitch));
        assert!(eq(t.origin_y, fit.top_row_origin_y));
    }
}

#[test]
fn pile_ids_and_counts_stable_across_all_arrangements() {
    let rules = Klondike::with_draw_count(3);
    for rect in [TOPROW_RECT, SIDECOL_RECT, STACKED_RECT] {
        let slots = rules.pile_layout(rect);
        assert_eq!(slots.len(), 13, "13 piles regardless of arrangement");
        for (i, s) in slots.iter().enumerate() {
            assert_eq!(s.id as usize, i, "ids stay contiguous 0..13 on {rect:?}");
        }
    }
}

#[test]
fn tall_rect_keeps_top_row_layout() {
    let rules = Klondike::with_draw_count(3);
    let slots = rules.pile_layout(Rect::new(0.0, 0.0, 390.0, 800.0));
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-6;
    // Pin the historical top-row layout: width-bound cards of
    // (390 - 6*12) / 7 = 45.428… wide, aspect 1.4.
    let card_w = 318.0 / 7.0;
    let card_h = card_w * crate::games::CARD_ASPECT;
    let col_pitch = card_w + 12.0;
    assert!(eq(slots[STOCK as usize].card_h, card_h));
    assert!(eq(slots[STOCK as usize].origin_x, 0.0));
    assert!(eq(slots[STOCK as usize].origin_y, 800.0 - card_h));
    let w = &slots[WASTE as usize];
    assert!(eq(w.origin_x, col_pitch));
    assert!(eq(w.origin_y, 800.0 - card_h));
    assert_eq!(w.fan_top_n, 3);
    assert!(eq(w.fan_dx, card_w * 0.27));
    assert!(eq(w.fan_dy, 0.0));
    // Foundations start in column 3 of the top row; tableau sits
    // one row-pitch below, starting at column 0.
    assert!(eq(slots[FOUND_FIRST as usize].origin_x, 3.0 * col_pitch));
    assert!(eq(slots[FOUND_FIRST as usize].origin_y, 800.0 - card_h));
    assert!(eq(slots[TABLEAU_FIRST as usize].origin_x, 0.0));
    assert!(eq(
        slots[TABLEAU_FIRST as usize].origin_y,
        800.0 - card_h - (card_h + 12.0)
    ));
}

#[test]
fn portrait_rect_scales_tableau_fans() {
    let rules = Klondike::new();
    let rect = Rect::new(0.0, 0.0, 390.0, 800.0);
    let slots = rules.pile_layout(rect);
    let scale = slots[TABLEAU_FIRST as usize].fan_scale;
    assert!(scale > 1.0, "portrait rect must stretch tableau fans");
    assert!(scale <= 2.0);
    for i in 0..COLS as u8 {
        assert_eq!(slots[(TABLEAU_FIRST + i) as usize].fan_scale, scale);
    }
    // Only tableau piles stretch — stock/waste/foundations keep 1.0
    // (and the waste's top-N fan offsets are untouched).
    assert_eq!(slots[STOCK as usize].fan_scale, 1.0);
    assert_eq!(slots[WASTE as usize].fan_scale, 1.0);
    for i in 0..4u8 {
        assert_eq!(slots[(FOUND_FIRST + i) as usize].fan_scale, 1.0);
    }
    // Worst-case column (6 face-down + K→A run of 13 face-up) must
    // still fit above the playfield bottom at this scale.
    let mut pile = crate::piles::Pile::from_slot(&slots[TABLEAU_FIRST as usize]);
    for _ in 0..6 {
        pile.cards
            .push(Card::new(crate::cards::Suit::Spades, Rank::King));
    }
    for _ in 0..13 {
        pile.cards
            .push(Card::new(crate::cards::Suit::Spades, Rank::King).face_up());
    }
    let (_, y_bottom) = pile.position_for(pile.cards.len() - 1);
    assert!(
        y_bottom >= rect.y,
        "worst-case pile bottom {y_bottom} overflows the playfield"
    );
}

#[test]
fn height_bound_rect_keeps_default_fan_scale() {
    let rules = Klondike::new();
    // A very wide, short rect makes height the binding dimension for
    // the winning arrangement, so there's no vertical slack to
    // stretch fans into.
    let slots = rules.pile_layout(Rect::new(0.0, 0.0, 2000.0, 500.0));
    for i in 0..COLS as u8 {
        let s = slots[(TABLEAU_FIRST + i) as usize].fan_scale;
        assert!(
            (s - 1.0).abs() < 1e-9,
            "height-bound fit must not stretch fans, got {s}"
        );
    }
}

#[test]
fn stock_click_when_empty_recycles_waste() {
    let rules = Klondike::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    for r in [Rank::Two, Rank::Three, Rank::Four] {
        piles
            .get_mut(WASTE)
            .cards
            .push(Card::new(Suit::Spades, r).face_up());
    }
    let moves = rules.on_pile_click(&piles, STOCK);
    let m = moves[0];
    assert!(rules.legal_move(&piles, &m));
    apply_move(&mut piles, &m);
    assert_eq!(piles.get(STOCK).len(), 3);
    assert_eq!(piles.get(WASTE).len(), 0);
    // After recycle, all stock cards face-down.
    assert!(piles.get(STOCK).cards.iter().all(|c| !c.face_up));
}

#[test]
fn initial_deal_fits_uncompressed_on_phone_rects() {
    // The fresh deal (deepest column: 6 face-down + 1 face-up) must
    // sit inside the tableau allowance at both phone orientations, so
    // no fan compression kicks in on turn one. 1 + 6·0.11 = 1.66
    // card-heights, well under the 2.8-card side budget.
    for rect in [
        Rect::new(0.0, 0.0, 908.0, 358.0),
        Rect::new(0.0, 0.0, 351.0, 740.0),
    ] {
        let mut s = GameSession::new(Klondike::new(), 7);
        s.relayout(rect);
        for id in TABLEAU_FIRST..=TABLEAU_LAST {
            let p = s.piles.get(id);
            let natural = p.layout.pile_height(p.card_h, p.fan_scale, &p.cards);
            assert!(
                natural <= p.max_fan_extent + 1e-6,
                "tableau {id} natural extent {natural} exceeds cap {} at {rect:?}",
                p.max_fan_extent
            );
        }
    }
}
