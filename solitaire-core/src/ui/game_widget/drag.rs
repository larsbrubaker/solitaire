//! Drag-and-drop plus click-move interaction for `GameWidget`. Split
//! out of `game_widget.rs` to keep that file under the 800-line limit;
//! lives in its own `impl GameWidget` block and reads the parent's
//! private fields via `super::`.

use agg_gui::draw_ctx::DrawCtx;

use crate::cards::Card;
use crate::piles::{HitResult, Pile, PileId, PileKind, FAN_DOWN_FACE_UP};
use crate::render::paint_card_at;
use crate::session::Move;
use crate::ui::app_model::Screen;
use crate::ui::dyn_session::DynGameSession;

use super::animations;
use super::GameWidget;

#[derive(Clone, Debug)]
pub(super) struct DragState {
    pub(super) source_pile: PileId,
    /// First card index in source pile that's part of the drag.
    pub(super) start_idx: usize,
    /// Snapshot of the cards being dragged (includes face_up flags).
    cards: Vec<Card>,
    /// Offset from the click point to the bottom-left of `cards[0]` in
    /// virtual coords. Lets the dragged stack follow the cursor at the
    /// same grab point through the whole motion.
    grab_dx: f64,
    grab_dy: f64,
    /// Latest cursor position in virtual coords.
    pub(super) cur_x: f64,
    pub(super) cur_y: f64,
    /// Initial pointer position; lets mouse-up distinguish a click from
    /// an intentional drag without preventing drag previews.
    pub(super) start_x: f64,
    pub(super) start_y: f64,
}

/// Two piles' card-slot rects overlap in screen space (Y-up). Used to
/// detect the stacked side-column layouts, where same-kind auxiliary
/// piles (Spider foundations, Klondike foundations, FreeCell cells /
/// foundations) sit at heavily overlapping origins.
fn slot_rects_overlap(a: &Pile, b: &Pile) -> bool {
    let (ax1, ay1) = (a.origin_x, a.origin_y);
    let (ax2, ay2) = (a.origin_x + a.card_w, a.origin_y + a.card_h);
    let (bx1, by1) = (b.origin_x, b.origin_y);
    let (bx2, by2) = (b.origin_x + b.card_w, b.origin_y + b.card_h);
    ax1 < bx2 && bx1 < ax2 && ay1 < by2 && by1 < ay2
}

/// Re-resolve a drop onto a Foundation or Cell when overlapping
/// same-kind slots made `PileSet::hit_test` pick a pile that rejects
/// the move.
///
/// The stacked side-column layouts pack same-kind auxiliary piles at
/// overlapping origins, so a drop in that column hit-tests to the
/// visually-topmost slot (highest id, since `hit_test` scans in reverse
/// — see `PileSet::hit_test`). That slot may reject a move the player
/// clearly intended: a completed Spider K→A run belongs in whatever
/// foundation is empty; an Ace belongs in whatever Klondike foundation
/// accepts it; a card dropped over a FULL topmost FreeCell cell belongs
/// in an empty one. Re-point the move at the first same-kind sibling
/// that legally accepts it.
///
/// Returns `m` unchanged unless the target is an overlapped Foundation
/// or Cell AND `m` is currently illegal — so an already-legal move is
/// never converted, and non-overlapping layouts (top-row boards, the
/// 2x2 side layouts) keep hit_test's authoritative result.
pub(super) fn resolve_overlapping_target(session: &dyn DynGameSession, m: Move) -> Move {
    let piles = session.piles();
    let target = piles.get(m.to);
    let kind = target.kind;
    if !matches!(kind, PileKind::Foundation | PileKind::Cell) || session.legal_move(&m) {
        return m;
    }
    let is_overlapped = piles
        .iter()
        .any(|p| p.kind == kind && p.id != m.to && slot_rects_overlap(target, p));
    if !is_overlapped {
        return m;
    }
    let sibling_ids: Vec<PileId> = piles
        .iter()
        .filter(|p| p.kind == kind && p.id != m.to)
        .map(|p| p.id)
        .collect();
    for sid in sibling_ids {
        let mut alt = m;
        alt.to = sid;
        if session.legal_move(&alt) {
            return alt;
        }
    }
    m
}

/// Window in which a second mouse-down at the same spot counts as a
/// double-click — used for the auto-send-to-foundation shortcut.
const DOUBLE_CLICK_MS: u128 = 350;
/// Maximum pointer drift between two clicks for them to be treated
/// as a double-click (in virtual pixels).
const DOUBLE_CLICK_RADIUS: f64 = 6.0;

impl GameWidget {
    pub(super) fn is_double_click(&self, vx: f64, vy: f64) -> bool {
        let Some((t, lx, ly)) = self.last_click else {
            return false;
        };
        t.elapsed().as_millis() <= DOUBLE_CLICK_MS
            && (lx - vx).abs() <= DOUBLE_CLICK_RADIUS
            && (ly - vy).abs() <= DOUBLE_CLICK_RADIUS
    }

    /// On a double-click of the topmost face-up card under the cursor,
    /// try every Foundation pile in declaration order and apply the
    /// first legal `src → foundation` move. Returns `true` if a move
    /// was applied.
    pub(super) fn try_double_click_to_foundation(&mut self, vx: f64, vy: f64) -> bool {
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return false;
        };
        let piles = session.piles();
        let Some(hit) = piles.hit_test(vx, vy) else {
            return false;
        };
        let HitResult::Card {
            pile: src,
            card_idx,
        } = hit
        else {
            return false;
        };
        // Only the topmost card is auto-moveable.
        let p = piles.get(src);
        if card_idx + 1 != p.cards.len() {
            return false;
        }
        let top = &p.cards[card_idx];
        if !top.face_up {
            return false;
        }
        // Cancel any in-flight drag started by the first click of the pair.
        self.drag = None;
        // Snapshot foundation pile ids while we still hold an immutable
        // borrow of `piles` via `session.piles()`. After this block we
        // can call `session.try_apply` (which takes &mut self) without
        // tripping the borrow checker.
        let foundation_ids: Vec<PileId> = piles
            .iter()
            .filter(|p| p.kind == PileKind::Foundation && p.id != src)
            .map(|p| p.id)
            .collect();
        // Match the drag-drop branch: if removing the topmost card would
        // expose a face-down tableau card underneath, flip it. Without
        // this, double-clicking a card off auto-foundation leaves the
        // newly-revealed card face-down.
        let from_pile_kind = p.kind;
        let beneath_face_down =
            from_pile_kind == PileKind::Tableau && card_idx > 0 && !p.cards[card_idx - 1].face_up;
        for dst in foundation_ids {
            let mut m = Move::simple(src, 1, dst);
            if beneath_face_down {
                m = m.with_flip_source();
            }
            if session.legal_move(&m) {
                let Some(records) = session.try_apply_recording(m) else {
                    continue;
                };
                let now = web_time::Instant::now();
                animations::queue_recorded_move_animations(
                    &mut self.animations,
                    &records,
                    now,
                    true,
                );
                model.spider_hint = None;
                agg_gui::animation::request_draw();
                return true;
            }
        }
        false
    }

    pub(super) fn is_moms(&self) -> bool {
        self.model
            .borrow()
            .session
            .as_ref()
            .map(|s| s.game_slug() == "moms")
            .unwrap_or(false)
    }

    /// Mom's-Solitaire click handler. Looks up the clicked cell and
    /// asks `games::moms::resolve_click` what to do with it (start a
    /// king-pickup, fire a swap, or ignore). The session's normal
    /// `try_apply` runs the swap so undo / win-detection just work.
    pub(super) fn try_moms_click(&mut self, vx: f64, vy: f64) -> bool {
        use crate::games::moms::{resolve_click, ClickResolution};
        let mut model = self.model.borrow_mut();
        let waiting = model.moms_waiting_king_at;
        // Phase 1: pure inspection — read piles, decide what the click
        // means. Borrow ends with `resolution`.
        let resolution = {
            let Some(session) = model.session.as_ref() else {
                return false;
            };
            let hit = session.piles().hit_test(vx, vy);
            let pile_id = match hit {
                Some(crate::piles::HitResult::Card { pile, .. }) => pile,
                Some(crate::piles::HitResult::EmptySlot { pile }) => pile,
                None => return false,
            };
            resolve_click(session.piles(), pile_id, waiting)
        };
        // Phase 2: apply mutation; the swap may also report a win.
        match resolution {
            ClickResolution::Ignored => false,
            ClickResolution::StartWaitingForKing(gap) => {
                model.moms_waiting_king_at = Some(gap);
                agg_gui::animation::request_draw();
                true
            }
            ClickResolution::ApplySwap(m) => {
                let won = {
                    let Some(session) = model.session.as_mut() else {
                        return false;
                    };
                    let Some(records) = session.try_apply_recording(m) else {
                        return false;
                    };
                    animations::queue_recorded_move_animations(
                        &mut self.animations,
                        &records,
                        web_time::Instant::now(),
                        true,
                    );
                    session.is_won()
                };
                model.moms_waiting_king_at = None;
                if won {
                    model.screen = Screen::Won;
                }
                agg_gui::animation::request_draw();
                true
            }
        }
    }

    pub(super) fn try_start_drag(&mut self, vx: f64, vy: f64) -> bool {
        let model = self.model.borrow();
        let Some(session) = model.session.as_ref() else {
            return false;
        };
        let piles = session.piles();
        let Some(hit) = piles.hit_test(vx, vy) else {
            return false;
        };
        match hit {
            HitResult::EmptySlot { .. } => false,
            HitResult::Card { pile, card_idx } => {
                let p = piles.get(pile);
                let card = &p.cards[card_idx];
                if !card.face_up {
                    return false;
                }
                // For tableau piles you can pick up multiple cards (the
                // whole tail starting at card_idx). For other pile kinds
                // only the topmost card is draggable.
                let take_from = match p.kind {
                    PileKind::Tableau => card_idx,
                    _ => p.cards.len() - 1,
                };
                // The full tail must form a valid drag candidate. Rules
                // engine will reject if the run is invalid on drop, but
                // we still allow the drag visually.
                let cards: Vec<Card> = p.cards[take_from..].to_vec();
                let (cx, cy, _, _) = p.card_rect(take_from);
                self.drag = Some(DragState {
                    source_pile: pile,
                    start_idx: take_from,
                    cards,
                    grab_dx: vx - cx,
                    grab_dy: vy - cy,
                    cur_x: vx,
                    cur_y: vy,
                    start_x: vx,
                    start_y: vy,
                });
                drop(model); // release borrow before request_draw side-effects
                agg_gui::animation::request_draw();
                true
            }
        }
    }

    pub(super) fn finish_drag(&mut self, vx: f64, vy: f64) {
        let Some(drag) = self.drag.take() else { return };
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return;
        };
        // Find the destination pile under the cursor (use the hit_test on
        // the dragged-card position rather than raw cursor — we anchor
        // off the dragged card[0]'s would-be origin). Card size comes
        // from the source pile (Mom's uses a different size than the
        // others).
        let src_pile = session.piles().get(drag.source_pile);
        let card_w = src_pile.card_w;
        let card_h = src_pile.card_h;
        let drag_card_x = vx - drag.grab_dx;
        let drag_card_y = vy - drag.grab_dy;
        let probe_x = drag_card_x + card_w / 2.0;
        let probe_y = drag_card_y + card_h / 2.0;
        let target = session.piles().hit_test(probe_x, probe_y);
        let target_pile: Option<PileId> = match target {
            Some(HitResult::Card { pile, .. }) => Some(pile),
            Some(HitResult::EmptySlot { pile }) => Some(pile),
            None => None,
        };
        let mut applied = false;
        if let Some(to) = target_pile {
            if to != drag.source_pile {
                let take = drag.cards.len() as u8;
                let from_pile = session.piles().get(drag.source_pile);
                // Mom's Solitaire's drag operation is a swap (the gap
                // and the card you drag onto it exchange places); every
                // other variant is a normal stack move with optional
                // auto-flip of the newly-exposed source card.
                let m = if session.game_slug() == "moms" {
                    Move::swap(drag.source_pile, to)
                } else {
                    let mut m = Move::simple(drag.source_pile, take, to);
                    if from_pile.kind == PileKind::Tableau && drag.start_idx > 0 {
                        let beneath = &from_pile.cards[drag.start_idx - 1];
                        if !beneath.face_up {
                            m = m.with_flip_source();
                        }
                    }
                    m
                };
                let m = resolve_overlapping_target(&**session, m);
                if let Some(records) = session.try_apply_recording(m) {
                    let now = web_time::Instant::now();
                    animations::queue_recorded_move_animations(
                        &mut self.animations,
                        &records,
                        now,
                        false,
                    );
                    applied = true;
                }
            }
        }
        // Win check.
        if session.is_won() {
            model.screen = Screen::Won;
        }
        if applied {
            model.spider_hint = None;
        }
        agg_gui::animation::request_draw();
    }

    pub(super) fn try_single_click_move(&mut self, source_pile: PileId, start_idx: usize) -> bool {
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return false;
        };
        let Some(m) = session.single_click_move(source_pile, start_idx) else {
            return false;
        };
        let Some(records) = session.try_apply_recording(m) else {
            return false;
        };
        let now = web_time::Instant::now();
        animations::queue_recorded_move_animations(&mut self.animations, &records, now, true);
        if session.is_won() {
            model.screen = Screen::Won;
        }
        model.spider_hint = None;
        agg_gui::animation::request_draw();
        true
    }

    pub(super) fn paint_dragged(&self, ctx: &mut dyn DrawCtx, drag: &DragState) {
        let bx = drag.cur_x - drag.grab_dx;
        let by = drag.cur_y - drag.grab_dy;
        // Use the source pile's card dimensions so a Mom's Solitaire
        // card stays sized correctly while floating at the cursor.
        let model = self.model.borrow();
        let (card_w, card_h, fan) = model
            .session
            .as_ref()
            .map(|s| {
                let p = s.piles().get(drag.source_pile);
                // Fan the floating stack by the SOURCE pile's effective
                // per-card step — the `position_for` delta already folds in
                // both `fan_scale` and `max_fan_extent` compression. A run
                // dragged off a compressed pile then spreads exactly as wide
                // as where it snaps back, instead of the wider uncompressed
                // `card_h * FAN_DOWN_FACE_UP * fan_scale`. The dragged cards
                // are still present in the pile during the drag, so
                // consecutive indices exist for a multi-card run; a single
                // card never uses the step.
                let step = if drag.start_idx + 1 < p.cards.len() {
                    let (_, y0) = p.position_for(drag.start_idx);
                    let (_, y1) = p.position_for(drag.start_idx + 1);
                    (y0 - y1).abs()
                } else {
                    p.card_h * FAN_DOWN_FACE_UP * p.fan_scale
                };
                (p.card_w, p.card_h, step)
            })
            .unwrap_or((90.0, 126.0, 126.0 * FAN_DOWN_FACE_UP));
        drop(model);
        for (i, card) in drag.cards.iter().enumerate() {
            let y = by - i as f64 * fan;
            paint_card_at(ctx, card, bx, y, card_w, card_h, &self.atlas);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{Rank, Suit};
    use crate::games::{spider::Spider, GameRules};
    use crate::piles::PileSet;
    use crate::session::GameSession;
    use agg_gui::geometry::Rect;

    // Spider pile-id layout: foundations 0..=7, stock 8, cascades 9..=18.
    const FOUND_FIRST: PileId = 0;
    const CASCADE_FIRST: PileId = 9;

    fn king_to_ace(suit: Suit) -> Vec<Card> {
        [
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
        ]
        .iter()
        .map(|&r| Card::new(suit, r).face_up())
        .collect()
    }

    fn spider_session(rect: Rect) -> GameSession<Spider> {
        let mut s = GameSession::new(Spider::four_suit(), 1);
        s.piles = PileSet::from_slots(&Spider::four_suit().pile_layout(rect));
        s
    }

    #[test]
    fn side_column_run_to_foundation_skips_filled_lower_foundations() {
        // Wide rect -> side-column layout: the 8 foundations stack at
        // overlapping origins, so a drop anywhere in that column
        // hit-tests to the visually-topmost (HIGHEST-id) foundation —
        // `PileSet::hit_test` scans in reverse. This test drives the
        // resolver directly with a low-id target to exercise its
        // forward-scan retarget over several filled foundations.
        let mut s = spider_session(Rect::new(0.0, 0.0, 1600.0, 700.0));
        // found[0..=2] already hold completed suits.
        for fid in FOUND_FIRST..FOUND_FIRST + 3 {
            for c in king_to_ace(Suit::Spades) {
                s.piles.get_mut(fid).cards.push(c);
            }
        }
        // A complete K->A run sits on a cascade, ready to be dragged off.
        let cid = CASCADE_FIRST;
        for c in king_to_ace(Suit::Hearts) {
            s.piles.get_mut(cid).cards.push(c);
        }
        // The aimed foundation (found[0]) is occupied; the resolver must
        // re-point the run at the first EMPTY foundation (found[3]).
        let base = Move::simple(cid, 13, FOUND_FIRST);
        let resolved = resolve_overlapping_target(&s, base);
        assert_eq!(
            resolved.to,
            FOUND_FIRST + 3,
            "must retarget to the first empty foundation"
        );
        assert!(
            s.try_apply_recording(resolved).is_some(),
            "re-pointed run drop must apply"
        );
        assert!(s.piles.get(cid).is_empty(), "run left the cascade");
        assert_eq!(s.piles.get(FOUND_FIRST + 3).len(), 13);
    }

    #[test]
    fn side_column_run_retargets_from_occupied_highest_id_foundation() {
        // The scenario reverse-order hit-testing actually produces: a drop
        // in the overlapping foundation column resolves to the HIGHEST-id
        // foundation. When THAT slot is the occupied one, the resolver must
        // retarget forward to the first empty sibling (found[0]).
        let mut s = spider_session(Rect::new(0.0, 0.0, 1600.0, 700.0));
        // Only the highest-id foundation (found[7]) is full.
        let occupied = FOUND_FIRST + 7;
        for c in king_to_ace(Suit::Spades) {
            s.piles.get_mut(occupied).cards.push(c);
        }
        let cid = CASCADE_FIRST;
        for c in king_to_ace(Suit::Hearts) {
            s.piles.get_mut(cid).cards.push(c);
        }
        // hit_test picked the occupied found[7]; the resolver re-points at
        // the first EMPTY foundation (found[0]).
        let base = Move::simple(cid, 13, occupied);
        let resolved = resolve_overlapping_target(&s, base);
        assert_eq!(
            resolved.to, FOUND_FIRST,
            "must retarget off the occupied highest-id foundation to the first empty one"
        );
        assert!(
            s.try_apply_recording(resolved).is_some(),
            "re-pointed run drop must apply"
        );
        assert!(s.piles.get(cid).is_empty(), "run left the cascade");
        assert_eq!(s.piles.get(FOUND_FIRST).len(), 13);
    }

    #[test]
    fn top_row_run_to_foundation_uses_hit_tested_target() {
        // Tall rect -> classic top-row layout: foundations do NOT overlap,
        // so hit_test already returns the aimed (empty) foundation and the
        // resolver is a no-op.
        let mut s = spider_session(Rect::new(0.0, 0.0, 390.0, 800.0));
        for c in king_to_ace(Suit::Spades) {
            s.piles.get_mut(FOUND_FIRST).cards.push(c);
        }
        let cid = CASCADE_FIRST;
        for c in king_to_ace(Suit::Hearts) {
            s.piles.get_mut(cid).cards.push(c);
        }
        // Player aims at the empty found[1]; hit_test returns it directly.
        let base = Move::simple(cid, 13, FOUND_FIRST + 1);
        let resolved = resolve_overlapping_target(&s, base);
        assert_eq!(
            resolved.to,
            FOUND_FIRST + 1,
            "no retarget in the non-overlapping top-row layout"
        );
        assert!(s.try_apply_recording(resolved).is_some());
        assert!(s.piles.get(cid).is_empty());
        assert_eq!(s.piles.get(FOUND_FIRST + 1).len(), 13);
    }

    #[test]
    fn resolver_ignores_non_foundation_targets() {
        // A cascade->cascade drop must pass through untouched even when
        // it happens to be illegal (the caller handles rejection).
        let s = spider_session(Rect::new(0.0, 0.0, 1600.0, 700.0));
        let base = Move::simple(CASCADE_FIRST, 1, CASCADE_FIRST + 1);
        let resolved = resolve_overlapping_target(&s, base);
        assert_eq!(resolved, base);
    }
}
