use super::*;
use crate::cards::Suit;
use crate::session::GameSession;

#[test]
fn deal_distributes_52_cards_across_8_cascades() {
    let s = GameSession::new(FreeCell::new(), 1);
    let total: usize = (CASCADE_FIRST..=CASCADE_LAST)
        .map(|id| s.piles.get(id).len())
        .sum();
    assert_eq!(total, 52);
    for id in CASCADE_FIRST..=CASCADE_FIRST + 3 {
        assert_eq!(s.piles.get(id).len(), 7);
    }
    for id in CASCADE_FIRST + 4..=CASCADE_LAST {
        assert_eq!(s.piles.get(id).len(), 6);
    }
    // All cards face-up.
    for id in CASCADE_FIRST..=CASCADE_LAST {
        assert!(s.piles.get(id).cards.iter().all(|c| c.face_up));
    }
}

#[test]
fn cell_accepts_single_card() {
    let rules = FreeCell::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(CASCADE_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Two).face_up());
    let m = Move::simple(CASCADE_FIRST, 1, CELL_FIRST);
    assert!(rules.legal_move(&piles, &m));
}

#[test]
fn cell_rejects_when_full() {
    let rules = FreeCell::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(CELL_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::King).face_up());
    piles
        .get_mut(CASCADE_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Two).face_up());
    let m = Move::simple(CASCADE_FIRST, 1, CELL_FIRST);
    assert!(!rules.legal_move(&piles, &m));
}

#[test]
fn single_click_top_card_prefers_foundation() {
    let rules = FreeCell::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    piles
        .get_mut(CASCADE_FIRST)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Ace).face_up());
    piles
        .get_mut(CASCADE_FIRST + 1)
        .cards
        .push(Card::new(Suit::Spades, Rank::Two).face_up());

    let m = rules
        .single_click_move(&piles, CASCADE_FIRST, 0)
        .expect("ace can move to foundation");
    assert_eq!(m.from, CASCADE_FIRST);
    assert_eq!(m.to, FOUND_FIRST);
    assert_eq!(m.take, 1);
}

#[test]
fn single_click_run_moves_to_leftmost_legal_cascade() {
    let rules = FreeCell::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    let src = CASCADE_FIRST + 4;
    for id in CASCADE_FIRST..=CASCADE_LAST {
        if id == src {
            continue;
        }
        piles
            .get_mut(id)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
    }
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Clubs, Rank::Ten).face_up());
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Nine).face_up());

    let left_dst = CASCADE_FIRST + 1;
    let right_dst = CASCADE_FIRST + 3;
    for dst in [left_dst, right_dst] {
        piles.get_mut(dst).cards.clear();
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

// Rects at which each of the three candidate arrangements wins
// (8/12/10-column budgets 4.0/3.2/3.2):
//   TopRow      — tall portrait rect.
//   SideColumns — very wide, short rect (12- and 10-col sides tie on
//                 height; tie-break keeps the 2x2 layout).
//   SideStacked — moderately wide rect (10 cols beats 12 on width).
const TOPROW_RECT: Rect = Rect::new(0.0, 0.0, 390.0, 800.0);
const SIDECOL_RECT: Rect = Rect::new(0.0, 0.0, 2000.0, 500.0);
const STACKED_RECT: Rect = Rect::new(0.0, 0.0, 1600.0, 700.0);

#[test]
fn side_columns_2x2_wins_on_wide_short_rect() {
    let rules = FreeCell::new();
    let slots = rules.pile_layout(SIDECOL_RECT);
    let side = crate::games::fit_cards(SIDECOL_RECT, 12, 10.0, 12.0, 3.2);
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(eq(slots[CELL_FIRST as usize].card_h, side.card_h));
    // Free cells: 2x2 grid across columns 0 AND 1.
    for i in 0..4u8 {
        let c = &slots[(CELL_FIRST + i) as usize];
        assert!(eq(c.origin_x, side.left + (i % 2) as f64 * side.col_pitch));
        assert!(eq(
            c.origin_y,
            side.top_row_origin_y - (i / 2) as f64 * side.row_pitch
        ));
    }
    // Cascades: columns 2..=9, full playfield height.
    for i in 0..8u8 {
        let t = &slots[(CASCADE_FIRST + i) as usize];
        assert!(eq(t.origin_x, side.left + (2 + i) as f64 * side.col_pitch));
        assert!(eq(t.origin_y, side.top_row_origin_y));
    }
    // Foundations: 2x2 grid across columns 10 AND 11.
    for i in 0..4u8 {
        let f = &slots[(FOUND_FIRST + i) as usize];
        assert!(eq(
            f.origin_x,
            side.left + (10 + i % 2) as f64 * side.col_pitch
        ));
    }
    // cell[1] / found[1] step sideways — distinguishes 2x2 from
    // stacked.
    assert!(slots[(CELL_FIRST + 1) as usize].origin_x > slots[CELL_FIRST as usize].origin_x);
    assert!(slots[(FOUND_FIRST + 1) as usize].origin_x > slots[FOUND_FIRST as usize].origin_x);
}

#[test]
fn side_stacked_wins_on_moderately_wide_rect() {
    let rules = FreeCell::new();
    let slots = rules.pile_layout(STACKED_RECT);
    let fit = crate::games::fit_cards(STACKED_RECT, 10, 10.0, 12.0, 3.2);
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let top = crate::games::fit_cards(STACKED_RECT, 8, 10.0, 12.0, 4.0);
    let side2x2 = crate::games::fit_cards(STACKED_RECT, 12, 10.0, 12.0, 3.2);
    assert!(fit.card_h > top.card_h && fit.card_h > side2x2.card_h);
    assert!(eq(slots[CELL_FIRST as usize].card_h, fit.card_h));
    // Cells share column 0 and foundations column 9, both stepping
    // DOWN. RE-PINNED: the step spreads across the column height but is
    // now clamped to [0.28·card_h floor, READABLE_STACK_STEP·card_h cap]
    // so a stacked group stays compact at the column top instead of
    // spreading into a ladder. This tall column spreads to the cap.
    let min_step = fit.card_h * STACKED_STEP;
    let cap = fit.card_h * crate::games::READABLE_STACK_STEP;
    let step = ((STACKED_RECT.height - fit.card_h) / 3.0).clamp(min_step, cap);
    assert!(step > min_step, "tall column must spread past the floor");
    for i in 0..4u8 {
        let c = &slots[(CELL_FIRST + i) as usize];
        assert!(eq(c.origin_x, fit.left));
        assert!(eq(c.origin_y, fit.top_row_origin_y - i as f64 * step));
        // Only the first (lowest-id) cell of the group paints a
        // placeholder; the rest hide it so the column is one socket.
        assert_eq!(c.show_empty_slot, i == 0, "cell {i} show_empty_slot");
    }
    // Foundations all share column 9, same stacking.
    for i in 0..4u8 {
        let f = &slots[(FOUND_FIRST + i) as usize];
        assert!(eq(f.origin_x, fit.left + 9.0 * fit.col_pitch));
        assert!(eq(f.origin_y, fit.top_row_origin_y - i as f64 * step));
        // Same rule for the foundation group: first slot only.
        assert_eq!(f.show_empty_slot, i == 0, "foundation {i} show_empty_slot");
    }
    // Cascades: columns 1..=8, full playfield height.
    for i in 0..8u8 {
        let t = &slots[(CASCADE_FIRST + i) as usize];
        assert!(eq(t.origin_x, fit.left + (1 + i) as f64 * fit.col_pitch));
        assert!(eq(t.origin_y, fit.top_row_origin_y));
    }
}

#[test]
fn pile_ids_and_counts_stable_across_all_arrangements() {
    let rules = FreeCell::new();
    for rect in [TOPROW_RECT, SIDECOL_RECT, STACKED_RECT] {
        let slots = rules.pile_layout(rect);
        assert_eq!(slots.len(), 16, "16 piles regardless of arrangement");
        for (i, s) in slots.iter().enumerate() {
            assert_eq!(s.id as usize, i, "ids stay contiguous 0..16 on {rect:?}");
        }
    }
}

#[test]
fn tall_rect_keeps_top_row_layout() {
    let rules = FreeCell::new();
    let slots = rules.pile_layout(Rect::new(0.0, 0.0, 390.0, 800.0));
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-6;
    // Pin the historical top-row layout: width-bound cards of
    // (390 - 7*10) / 8 = 40 wide, aspect 1.4 → 56 tall.
    let card_w = 40.0;
    let card_h = card_w * crate::games::CARD_ASPECT;
    let col_pitch = card_w + 10.0;
    assert!(eq(slots[CELL_FIRST as usize].card_h, card_h));
    // Cells in columns 0..=3, foundations in columns 4..=7, all in
    // the top row; cascades one row-pitch below at column 0.
    assert!(eq(slots[CELL_FIRST as usize].origin_x, 0.0));
    assert!(eq(slots[CELL_FIRST as usize].origin_y, 800.0 - card_h));
    assert!(eq(slots[FOUND_FIRST as usize].origin_x, 4.0 * col_pitch));
    assert!(eq(slots[FOUND_FIRST as usize].origin_y, 800.0 - card_h));
    assert!(eq(slots[CASCADE_FIRST as usize].origin_x, 0.0));
    assert!(eq(
        slots[CASCADE_FIRST as usize].origin_y,
        800.0 - card_h - (card_h + 12.0)
    ));
}

#[test]
fn portrait_rect_scales_cascade_fans() {
    let rules = FreeCell::new();
    let rect = Rect::new(0.0, 0.0, 390.0, 800.0);
    let slots = rules.pile_layout(rect);
    let scale = slots[CASCADE_FIRST as usize].fan_scale;
    assert!(scale > 1.0, "portrait rect must stretch cascade fans");
    assert!(scale <= 2.0);
    for i in 0..N_CASCADES as u8 {
        assert_eq!(slots[(CASCADE_FIRST + i) as usize].fan_scale, scale);
    }
    // Only cascades stretch — cells and foundations keep 1.0.
    for i in 0..4u8 {
        assert_eq!(slots[(CELL_FIRST + i) as usize].fan_scale, 1.0);
        assert_eq!(slots[(FOUND_FIRST + i) as usize].fan_scale, 1.0);
    }
    // Worst-case cascade (19 face-up cards) must still fit above
    // the playfield bottom at this scale.
    let mut pile = crate::piles::Pile::from_slot(&slots[CASCADE_FIRST as usize]);
    for _ in 0..19 {
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
    let rules = FreeCell::new();
    // Wide, short rect → height binds for the winning arrangement,
    // leaving no vertical slack to stretch fans into.
    let slots = rules.pile_layout(Rect::new(0.0, 0.0, 2000.0, 500.0));
    for i in 0..N_CASCADES as u8 {
        let s = slots[(CASCADE_FIRST + i) as usize].fan_scale;
        assert!(
            (s - 1.0).abs() < 1e-9,
            "height-bound fit must not stretch fans, got {s}"
        );
    }
}

#[test]
fn multi_card_move_limited_by_empty_cells() {
    let rules = FreeCell::new();
    let mut piles = PileSet::from_slots(&rules.pile_layout(crate::session::DEFAULT_PLAYFIELD_RECT));
    // Fill every cell so empty_cells = 0.
    for id in CELL_FIRST..=CELL_LAST {
        piles
            .get_mut(id)
            .cards
            .push(Card::new(Suit::Hearts, Rank::King).face_up());
    }
    // Source cascade run: Q♠ J♥ 10♠ (alt-color descending).
    let src = CASCADE_FIRST;
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Spades, Rank::Queen).face_up());
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Hearts, Rank::Jack).face_up());
    piles
        .get_mut(src)
        .cards
        .push(Card::new(Suit::Spades, Rank::Ten).face_up());
    // Fill every other cascade so empty_cascades = 0 too.
    for id in (CASCADE_FIRST + 1)..=CASCADE_LAST {
        piles
            .get_mut(id)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
    }
    // Destination = CASCADE_FIRST + 1 (overwrite top so K♥ is on top).
    let dst = CASCADE_FIRST + 1;
    piles.get_mut(dst).cards.clear();
    piles
        .get_mut(dst)
        .cards
        .push(Card::new(Suit::Hearts, Rank::King).face_up());
    // Re-fill the remaining other cascades.
    for id in (CASCADE_FIRST + 2)..=CASCADE_LAST {
        // Already filled above; nothing to do.
        assert!(!piles.get(id).is_empty());
    }
    // With 0 empty cells AND 0 empty cascades, max_movable = 1
    // — moving 3 cards must fail.
    let m = Move::simple(src, 3, dst);
    assert!(!rules.legal_move(&piles, &m));
}

#[test]
fn initial_deal_fits_uncompressed_on_phone_rects() {
    // Deepest fresh cascade is 7 all-face-up cards: 1 + 6·0.176 =
    // 2.06 card-heights, under the 3.2-card side budget → no fan
    // compression on turn one, either orientation.
    for rect in [
        Rect::new(0.0, 0.0, 908.0, 358.0),
        Rect::new(0.0, 0.0, 351.0, 740.0),
    ] {
        let mut s = GameSession::new(FreeCell::new(), 1);
        s.relayout(rect);
        for id in CASCADE_FIRST..=CASCADE_LAST {
            let p = s.piles.get(id);
            let natural = p.layout.pile_height(p.card_h, p.fan_scale, &p.cards);
            assert!(
                natural <= p.max_fan_extent + 1e-6,
                "cascade {id} natural extent {natural} exceeds cap {} at {rect:?}",
                p.max_fan_extent
            );
        }
    }
}
