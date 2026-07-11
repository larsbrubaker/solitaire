//! `GameRules` — the seam between the four solitaire variants and the
//! shared engine. Each variant implements this trait; `GameWidget` holds
//! the active session behind a `DynGameSession` trait object.

pub mod freecell;
pub mod hint;
pub mod klondike;
pub mod klondike_hint;
pub mod klondike_solver;
pub mod moms;
pub mod ms_freecell;
pub mod seed_generator;
pub mod spider;
pub mod spider_reverse_gen;
pub mod spider_solver;
pub mod winnable_seeds;

#[cfg(test)]
mod deal_stability_tests;

use agg_gui::geometry::Rect;
use rand::rngs::StdRng;

use crate::piles::{PileId, PileSet, PileSlot};
use crate::session::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameKind {
    Klondike,
    FreeCell,
    Spider,
    MomsSolitaire,
}

impl GameKind {
    pub fn slug(self) -> &'static str {
        match self {
            GameKind::Klondike => "klondike",
            GameKind::FreeCell => "freecell",
            GameKind::Spider => "spider",
            GameKind::MomsSolitaire => "moms",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            GameKind::Klondike => "Klondike",
            GameKind::FreeCell => "FreeCell",
            GameKind::Spider => "Spider",
            GameKind::MomsSolitaire => "Mom's Solitaire",
        }
    }
}

/// Standard playing-card aspect ratio (height / width). Every variant
/// picks `card_h = card_w * CARD_ASPECT` so the on-screen cards keep a
/// recognisable shape regardless of available pixels.
pub const CARD_ASPECT: f64 = 7.0 / 5.0;

/// Shared board geometry computed by [`fit_cards`]. All values are in
/// the same SCREEN Y-up coordinates as the `rect` passed in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CardFit {
    pub card_w: f64,
    pub card_h: f64,
    /// Horizontal distance between successive column origins.
    pub col_pitch: f64,
    /// Vertical distance between successive row origins.
    pub row_pitch: f64,
    /// X of the leftmost column origin — the grid is centered
    /// horizontally inside the rect.
    pub left: f64,
    /// Y-up origin (bottom-left) of a card in the top row, which sits
    /// flush against the top of the rect.
    pub top_row_origin_y: f64,
}

/// Fit cards into `rect`: width-bound by `cols` columns separated by
/// `col_gap`, height-bound by `vert_budget_cards` card-heights plus one
/// `row_gap` (top row + worst-case tableau fan). The smaller of the two
/// candidate sizes wins so the whole board fits either way, and
/// `card_h / card_w` always equals [`CARD_ASPECT`].
pub(crate) fn fit_cards(
    rect: Rect,
    cols: usize,
    col_gap: f64,
    row_gap: f64,
    vert_budget_cards: f64,
) -> CardFit {
    let cols = cols as f64;
    let card_w_by_width = (rect.width - col_gap * (cols - 1.0)) / cols;
    let card_h_by_height = (rect.height - row_gap) / vert_budget_cards;
    let card_h = (card_w_by_width * CARD_ASPECT).min(card_h_by_height);
    let card_w = card_h / CARD_ASPECT;
    let used_w = cols * card_w + (cols - 1.0) * col_gap;
    CardFit {
        card_w,
        card_h,
        col_pitch: card_w + col_gap,
        row_pitch: card_h + row_gap,
        left: rect.x + (rect.width - used_w) / 2.0,
        top_row_origin_y: rect.y + rect.height - card_h,
    }
}

/// Which candidate board arrangement a game picked for the current
/// playfield rect. Variant order encodes the tie-break preference used
/// by [`pick_board_fit`]: earlier variants win when two candidates
/// yield the same card height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoardArrangement {
    /// Classic layout: stock/waste/cells/foundations in a row ABOVE
    /// the tableau.
    TopRow,
    /// Wide-viewport layout: auxiliary piles move into flanking side
    /// columns (2x2 grids, or Spider's single stacked foundation
    /// column) and the tableau spans the full playfield height.
    SideColumns,
    /// Like `SideColumns`, but each side's auxiliary piles collapse
    /// into a SINGLE overlapping column so the tableau gets one more
    /// full-width column. Fewer columns → wider cards when width binds.
    SideStacked,
}

impl BoardArrangement {
    /// `true` when the tableau spans the full playfield height (every
    /// side layout), `false` for the top-row layout where one card row
    /// plus a gap sits above the tableau. Drives the `available`
    /// vertical space in [`tableau_fan_scale`].
    fn tableau_full_height(self) -> bool {
        !matches!(self, BoardArrangement::TopRow)
    }
}

/// One candidate arrangement for [`pick_board_fit`]: its identity, its
/// tableau column count, and the vertical budget (in card-heights) used
/// to size cards by height.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoardCandidate {
    pub arrangement: BoardArrangement,
    pub cols: usize,
    pub vert_budget: f64,
}

/// Evaluate every candidate arrangement and pick whichever yields the
/// LARGER card. Ties go to the candidate listed EARLIER in `candidates`
/// (callers order them TopRow, then SideColumns, then SideStacked), so
/// a height-bound rect where several candidates size identically keeps
/// the simplest layout. Returns the winning fit, its arrangement, and
/// its vertical budget (needed by [`tableau_fan_scale`]).
pub(crate) fn pick_board_fit(
    rect: Rect,
    col_gap: f64,
    row_gap: f64,
    candidates: &[BoardCandidate],
) -> (CardFit, BoardArrangement, f64) {
    let mut best: Option<(CardFit, BoardArrangement, f64)> = None;
    for cand in candidates {
        let fit = fit_cards(rect, cand.cols, col_gap, row_gap, cand.vert_budget);
        // Strictly-greater replacement keeps the earlier (already-seen)
        // candidate on ties — the tie-break preference.
        let better = match best {
            Some((cur, _, _)) => fit.card_h > cur.card_h,
            None => true,
        };
        if better {
            best = Some((fit, cand.arrangement, cand.vert_budget));
        }
    }
    best.expect("pick_board_fit needs at least one candidate")
}

/// Fan-step scale for the tableau piles of the arrangement that
/// [`pick_board_fit`] chose with winning budget `budget`. When card
/// size was width-bound (portrait phones), the vertical budget the fit
/// reserved for the tableau under-uses the playfield — this returns how
/// much the fan steps can stretch to fill it, in `1.0..=2.0`.
///
/// Fan overflow no longer needs a cap here: `Pile::max_fan_extent`
/// compression (see `Pile::position_for`) guarantees a deep pile fits.
/// This is purely the slack-expansion factor. Card size itself must NOT
/// change with this scale — only fan spacing.
pub(crate) fn tableau_fan_scale(
    rect: Rect,
    fit: &CardFit,
    arrangement: BoardArrangement,
    row_gap: f64,
    budget: f64,
) -> f64 {
    let card_h = fit.card_h;
    // `fit_cards` sizes cards from `card_h_by_height = (rect.height -
    // row_gap) / budget`, i.e. when height binds `rect.height ==
    // row_gap + budget * card_h` exactly. Per arrangement:
    // - TopRow: one card row plus `row_gap` sits above the tableau, so
    //     available = rect.height - card_h - row_gap
    //     assumed   = (budget - 1.0) * card_h
    //   At the height-bound equality, available == assumed → scale 1.0.
    // - Side layouts: the tableau spans the full playfield height, so
    //     available = rect.height
    //     assumed   = budget * card_h + row_gap
    //   Again equal (scale exactly 1.0) when height binds.
    let (available, assumed) = if arrangement.tableau_full_height() {
        (rect.height, budget * card_h + row_gap)
    } else {
        (rect.height - card_h - row_gap, (budget - 1.0) * card_h)
    };
    // Degenerate playfields (e.g. a 96×84 native window mid-resize
    // yields card_h == 0 and available == 0) would turn the division
    // below into NaN, and `f64::clamp` PANICS on a NaN bound. Fall
    // back to the default scale instead.
    if card_h <= 0.0 {
        return 1.0;
    }
    let scale = available / assumed;
    if !scale.is_finite() {
        return 1.0;
    }
    // Never stretch beyond 2.0, never compress below the default 1.0
    // (fan compression handles the other end of the range).
    scale.clamp(1.0, 2.0)
}

/// Upper bound for the stacked side-column step, as a fraction of
/// `card_h`. A completed pile only needs a thin sliver of the one behind
/// it visible to read as a distinct card in the stack, so the step caps
/// here rather than at full card-plus-gap separation. This keeps a
/// stacked group compact at the column top: each new completed pile
/// lands adjacent to the previous one instead of spreading a short stack
/// across a mostly-empty column (which read as a ladder of empty slots).
pub(crate) const READABLE_STACK_STEP: f64 = 0.30;

/// Vertical step between successive origins for a stacked side-column
/// of `n` overlapping slots (Spider's foundation column, Klondike /
/// FreeCell's SideStacked cells & foundations). Instead of a fixed
/// fraction of `card_h`, the step SPREADS the stack into the column's
/// available height so a short stack doesn't cram against the top of a
/// mostly-empty column:
///
/// ```text
/// step = ((available_height - card_h) / (n - 1)).clamp(min_step, READABLE_STACK_STEP * card_h)
/// ```
///
/// The floor `min_step` (each game's former fixed step, in pixels) keeps
/// slots readable on cramped viewports; the cap
/// [`READABLE_STACK_STEP`]`* card_h` is a compact readable overlap —
/// just enough of each pile behind peeks out — beyond which we stop
/// spreading, so a stacked group sits tightly at the column top. The
/// stack stays TOP-ALIGNED at the column top (callers step downward from
/// `top_row_origin_y`); this returns only the step, never centers or
/// bottom-aligns.
///
/// Degenerate inputs (n < 2, non-finite / non-positive `card_h`,
/// non-finite `available_height`, or a non-finite raw step) fall back to
/// `min_step` so no NaN can reach the clamp (a prior review caught
/// exactly such a panic). The clamp bounds are ordered defensively so an
/// oversized `min_step` can never invert them.
pub(crate) fn stacked_side_step(
    available_height: f64,
    card_h: f64,
    n: usize,
    min_step: f64,
) -> f64 {
    if n < 2 || !card_h.is_finite() || card_h <= 0.0 || !available_height.is_finite() {
        return min_step;
    }
    let raw = (available_height - card_h) / (n as f64 - 1.0);
    if !raw.is_finite() {
        return min_step;
    }
    let lo = min_step;
    let hi = (READABLE_STACK_STEP * card_h).max(lo);
    raw.clamp(lo, hi)
}

pub trait GameRules: 'static {
    /// Compute pile positions, sizes, and rendering config for the
    /// given playfield rect in SCREEN coordinates. The game picks a
    /// card size that fits its column count + worst-case fan within
    /// `rect`, then places each pile's bottom-left card origin.
    /// `GameWidget` calls this on every viewport change and re-
    /// applies the result via `PileSet::update_layout` (card state
    /// is preserved).
    fn pile_layout(&self, rect: Rect) -> Vec<PileSlot>;
    fn deal(&self, piles: &mut PileSet, rng: &mut StdRng);
    fn legal_move(&self, piles: &PileSet, m: &Move) -> bool;
    fn auto_complete_step(&self, piles: &PileSet) -> Option<Move>;
    fn is_won(&self, piles: &PileSet) -> bool;
    fn game_slug(&self) -> &'static str;

    /// Click handler — used by stock-tap interactions (Klondike stock
    /// dispense/recycle; Spider broadcast-deal). Returns the
    /// sequence of moves to apply in order. Empty vec means the click
    /// produced no moves.
    fn on_pile_click(&self, _piles: &PileSet, _pile: PileId) -> Vec<Move> {
        Vec::new()
    }

    /// Single-click card move helper. Variants that support click-to-
    /// move can return the move they want applied for `card_idx` in
    /// `pile`; the UI only calls this for a press/release with no drag.
    fn single_click_move(&self, _piles: &PileSet, _pile: PileId, _card_idx: usize) -> Option<Move> {
        None
    }

    /// After-move hook — called by `GameSession::try_apply` in a loop
    /// after every user move. Used by Spider to collapse complete K→A
    /// suited runs into a foundation. Returns `None` when no auto step
    /// applies.
    fn after_move(&self, _piles: &PileSet) -> Option<Move> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Degenerate playfield rects (mid-resize native windows can hand
    /// us near-zero heights) must not panic in `tableau_fan_scale`'s
    /// clamps and must fall back to the default fan scale. The 72×12
    /// rect is the playfield of a 96×84 native window (48 px HUD
    /// strip + 12 px pads) and reproduces the exact `0.0 / 0.0 = NaN`
    /// case: TopRow with `rect.height == row_gap` gives both
    /// `card_h == 0.0` and `available == 0.0`, and `f64::clamp`
    /// panics on a NaN bound.
    #[test]
    fn degenerate_rects_do_not_panic_and_keep_default_fan_scale() {
        let games: [Box<dyn GameRules>; 3] = [
            Box::new(klondike::Klondike::new()),
            Box::new(freecell::FreeCell::new()),
            Box::new(spider::Spider::four_suit()),
        ];
        for rect in [
            Rect::new(0.0, 0.0, 0.0, 0.0),
            Rect::new(0.0, 0.0, 72.0, 12.0),
        ] {
            for g in &games {
                let slots = g.pile_layout(rect);
                for (i, s) in slots.iter().enumerate() {
                    assert!(
                        s.fan_scale == 1.0,
                        "{} slot {i} fan_scale {} on degenerate rect {rect:?}",
                        g.game_slug(),
                        s.fan_scale
                    );
                    // `max_fan_extent` can go zero or negative on these
                    // rects (e.g. rect.height - card_h - row_gap). Fan
                    // compression must still resolve positions without a
                    // NaN reaching a clamp/division — build a pile from
                    // the slot, deal a few cards, and walk every index.
                    let mut pile = crate::piles::Pile::from_slot(s);
                    for _ in 0..8 {
                        pile.cards.push(crate::cards::Card::new(
                            crate::cards::Suit::Spades,
                            crate::cards::Rank::King,
                        ));
                    }
                    for idx in 0..pile.cards.len() {
                        let (x, y) = pile.position_for(idx);
                        assert!(
                            x.is_finite() && y.is_finite(),
                            "{} slot {i} position_for({idx}) = ({x}, {y}) on rect {rect:?}",
                            g.game_slug(),
                        );
                    }
                }
            }
        }
    }

    /// A cramped column (available height barely more than a card) can't
    /// spread the slots, so the step pins to the `min_step` floor.
    #[test]
    fn stacked_side_step_cramped_column_uses_floor() {
        let card_h = 100.0;
        let min_step = card_h * 0.28;
        // available - card_h = 30 → raw = 30/3 = 10 < 28 floor.
        let step = stacked_side_step(card_h + 30.0, card_h, 4, min_step);
        assert!(
            (step - min_step).abs() < 1e-9,
            "cramped step {step} != floor"
        );
    }

    /// A tall column spreads the slots but never beyond the readable cap
    /// (`READABLE_STACK_STEP * card_h`), so a stacked group stays compact
    /// at the column top rather than spreading into a ladder.
    #[test]
    fn stacked_side_step_tall_column_caps_at_readable_step() {
        let card_h = 100.0;
        let min_step = card_h * 0.15;
        // Huge available height → raw is enormous → clamps to the cap.
        let step = stacked_side_step(10_000.0, card_h, 4, min_step);
        assert!(
            (step - READABLE_STACK_STEP * card_h).abs() < 1e-9,
            "step {step} not capped at readable step {}",
            READABLE_STACK_STEP * card_h
        );
    }

    /// Between floor and cap the step spreads, origins step monotonically
    /// downward, and the stack's extent never exceeds the column height.
    #[test]
    fn stacked_side_step_spreads_monotonic_within_column() {
        let card_h = 90.0;
        let available = 358.0;
        let n = 8;
        let min_step = card_h * 0.15;
        let step = stacked_side_step(available, card_h, n, min_step);
        assert!(step > min_step, "step {step} should spread past floor");
        assert!(
            step <= READABLE_STACK_STEP * card_h + 1e-9,
            "step {step} exceeds readable cap"
        );
        // Origins step strictly downward from the top (top-aligned).
        let top = available; // arbitrary top origin
        let mut prev = top + 1.0;
        for i in 0..n {
            let y = top - i as f64 * step;
            assert!(y < prev, "origins not monotonic at slot {i}");
            prev = y;
        }
        // Extent (top slot's top edge down to bottom slot's origin) fits.
        let extent = card_h + (n as f64 - 1.0) * step;
        assert!(
            extent <= available + 1e-9,
            "extent {extent} exceeds column {available}"
        );
    }

    /// Degenerate inputs fall back to `min_step` with no NaN reaching the
    /// clamp (mirrors the `tableau_fan_scale` panic guard).
    #[test]
    fn stacked_side_step_degenerate_inputs_fall_back_to_floor() {
        let min_step = 20.0;
        // n < 2, non-finite / non-positive card_h, non-finite height.
        assert_eq!(stacked_side_step(500.0, 100.0, 1, min_step), min_step);
        assert_eq!(stacked_side_step(500.0, 0.0, 4, min_step), min_step);
        assert_eq!(stacked_side_step(500.0, -5.0, 4, min_step), min_step);
        assert_eq!(stacked_side_step(500.0, f64::NAN, 4, min_step), min_step);
        assert_eq!(stacked_side_step(f64::NAN, 100.0, 4, min_step), min_step);
        assert_eq!(
            stacked_side_step(f64::INFINITY, 100.0, 4, min_step),
            min_step
        );
    }
}
