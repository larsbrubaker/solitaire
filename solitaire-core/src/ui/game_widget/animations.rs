//! Animation queue helpers and per-frame paint glue.
//!
//! Extracted from `game_widget.rs` to keep that file under the 800-line
//! limit. Everything in here is stateless: the queue builders take the
//! existing `Vec<CardAnim>` by `&mut`, do the `PileSet` reads, and
//! push frames. The paint helpers take an atlas + an `&CardAnim` and
//! emit the corresponding `draw_image_rgba_corners` call.

use std::time::Duration;

use agg_gui::draw_ctx::DrawCtx;
use web_time::Instant;

use crate::cards::Card;
use crate::piles::{PileId, PileKind, PileSet};
use crate::render::CardSpriteAtlas;
use crate::session::{AppliedMoveRecord, Move};

use crate::ui::animation::{animated_quad, CardAnim, DeckFlipAnim};

pub(super) fn is_source_reveal_anim(anim: &CardAnim) -> bool {
    anim.hold_before_start
        && anim.flip
        && (anim.src_x - anim.dst_x).abs() < 0.5
        && (anim.src_y - anim.dst_y).abs() < 0.5
}

pub(super) fn paint_card_animation(
    ctx: &mut dyn DrawCtx,
    atlas: &CardSpriteAtlas,
    anim: &CardAnim,
) {
    let q = animated_quad(anim);
    let sprite = if q.show_front {
        atlas.face(anim.card.suit, anim.card.rank)
    } else {
        atlas.back()
    };
    ctx.draw_image_rgba_corners(&sprite, atlas.px_w, atlas.px_h, q.corners);
}

/// `hide_from` index for `pile_id` while any in-flight animation
/// targets it — `None` means draw all cards normally. Multiple
/// animations targeting the same pile collapse to the lowest
/// `dst_hide_from` so all of them are correctly suppressed.
pub(super) fn hide_from_for(
    animations: &[CardAnim],
    deck_animations: &[DeckFlipAnim],
    pile_id: PileId,
) -> Option<usize> {
    let card_min = animations
        .iter()
        .filter(|a| a.should_paint_now())
        .filter(|a| a.dst_pile == pile_id)
        .map(|a| a.dst_hide_from)
        .min();
    let deck_min = deck_animations
        .iter()
        .filter(|a| a.has_started())
        .filter(|a| a.dst_pile == pile_id)
        .map(|a| a.dst_hide_from)
        .min();
    match (card_min, deck_min) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

pub(super) fn queue_source_flip_animation_from_piles(
    animations: &mut Vec<CardAnim>,
    piles: &PileSet,
    m: &Move,
    now: Instant,
) {
    if !m.flip_source_after {
        return;
    }
    let pile = piles.get(m.from);
    let Some(idx) = pile.cards.len().checked_sub(1) else {
        return;
    };
    let card = pile.cards[idx];
    if !card.face_up {
        return;
    }
    let (x, y) = pile.position_for(idx);
    animations.push(CardAnim {
        card,
        src_x: x,
        src_y: y,
        dst_x: x,
        dst_y: y,
        card_w: pile.card_w,
        card_h: pile.card_h,
        start_at: now,
        duration: Duration::from_millis(260),
        dst_pile: m.from,
        dst_hide_from: idx,
        flip: true,
        hold_before_start: true,
        late_appear_at: None,
    });
}

pub(super) fn snapshot_move_sources(piles: &PileSet, m: &Move) -> Vec<(Card, f64, f64)> {
    if m.swap_with_top || m.take == 0 {
        return Vec::new();
    }
    let pile = piles.get(m.from);
    let take = m.take as usize;
    if pile.cards.len() < take {
        return Vec::new();
    }
    let start = pile.cards.len() - take;
    (start..pile.cards.len())
        .map(|idx| {
            let (x, y) = pile.position_for(idx);
            (pile.cards[idx], x, y)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn queue_move_animations_from_snapshot(
    animations: &mut Vec<CardAnim>,
    sources: &[(Card, f64, f64)],
    piles: &PileSet,
    m: &Move,
    now: Instant,
    stagger: Duration,
    hold_before_start: bool,
    late_hold: Option<(usize, Instant)>,
) {
    if sources.is_empty() || m.swap_with_top {
        return;
    }
    let take = m.take as usize;
    let dst_pile = piles.get(m.to);
    if dst_pile.cards.len() < take {
        return;
    }
    let dst_start = dst_pile.cards.len() - take;
    for dst_offset in 0..take {
        let src_offset = if m.reverse_order {
            take - 1 - dst_offset
        } else {
            dst_offset
        };
        let Some((_, src_x, src_y)) = sources.get(src_offset).copied() else {
            continue;
        };
        let dst_idx = dst_start + dst_offset;
        let card = dst_pile.cards[dst_idx];
        let (dst_x, dst_y) = dst_pile.position_for(dst_idx);
        let late_appear_at = match late_hold {
            Some((late_top, late_at))
                if hold_before_start && src_offset + late_top >= sources.len() =>
            {
                Some(late_at)
            }
            _ => None,
        };
        animations.push(CardAnim {
            card,
            src_x,
            src_y,
            dst_x,
            dst_y,
            card_w: dst_pile.card_w,
            card_h: dst_pile.card_h,
            start_at: now + stagger * dst_offset as u32,
            duration: Duration::from_millis(480),
            dst_pile: m.to,
            dst_hide_from: dst_start,
            flip: m.flip_moved,
            hold_before_start,
            late_appear_at,
        });
    }
}

pub(super) fn snapshot_swap_sources(
    piles: &PileSet,
    m: &Move,
) -> Option<[(PileId, Card, f64, f64); 2]> {
    if !m.swap_with_top {
        return None;
    }
    let from = piles.get(m.from);
    let to = piles.get(m.to);
    let from_idx = from.cards.len().checked_sub(1)?;
    let to_idx = to.cards.len().checked_sub(1)?;
    let (from_x, from_y) = from.position_for(from_idx);
    let (to_x, to_y) = to.position_for(to_idx);
    Some([
        (m.from, from.cards[from_idx], from_x, from_y),
        (m.to, to.cards[to_idx], to_x, to_y),
    ])
}

pub(super) fn queue_swap_animations_from_snapshot(
    animations: &mut Vec<CardAnim>,
    sources: &[(PileId, Card, f64, f64); 2],
    piles: &PileSet,
    m: &Move,
    now: Instant,
) {
    if !m.swap_with_top {
        return;
    }
    let from_kind = piles.get(m.from).kind;
    let to_kind = piles.get(m.to).kind;
    let moms_gap_swap = from_kind == PileKind::Tableau
        && to_kind == PileKind::Tableau
        && sources
            .iter()
            .any(|(_, card, _, _)| card.rank == crate::cards::Rank::Ace);

    for &(src_pile, card, src_x, src_y) in sources {
        if moms_gap_swap && card.rank == crate::cards::Rank::Ace {
            continue;
        }
        let dst_id = if src_pile == m.from { m.to } else { m.from };
        let dst_pile = piles.get(dst_id);
        let Some(dst_idx) = dst_pile.cards.len().checked_sub(1) else {
            continue;
        };
        let (dst_x, dst_y) = dst_pile.position_for(dst_idx);
        animations.push(CardAnim {
            card,
            src_x,
            src_y,
            dst_x,
            dst_y,
            card_w: dst_pile.card_w,
            card_h: dst_pile.card_h,
            start_at: now,
            duration: Duration::from_millis(220),
            dst_pile: dst_id,
            dst_hide_from: dst_idx,
            flip: false,
            hold_before_start: false,
            late_appear_at: None,
        });
    }
}

pub(super) fn is_klondike_stock_to_waste_draw(piles: &PileSet, m: &Move) -> bool {
    m.flip_moved
        && !m.reverse_order
        && !m.swap_with_top
        && piles.get(m.from).kind == PileKind::Stock
        && piles.get(m.to).kind == PileKind::Waste
}

pub(super) fn snapshot_waste_fan_compaction(
    piles: &PileSet,
    waste_id: PileId,
) -> Vec<(usize, Card, f64, f64)> {
    let waste = piles.get(waste_id);
    if waste.fan_top_n == 0 || waste.fan_dx == 0.0 || waste.cards.is_empty() {
        return Vec::new();
    }
    let top_n = (waste.fan_top_n as usize).min(waste.cards.len());
    let start = waste.cards.len() - top_n;
    (start..waste.cards.len())
        .map(|idx| {
            let (x, y) = waste.position_for(idx);
            (idx, waste.cards[idx], x, y)
        })
        .collect()
}

pub(super) fn snapshot_source_waste_reflow(
    piles: &PileSet,
    m: &Move,
) -> Vec<(usize, Card, f64, f64)> {
    if m.swap_with_top || m.reverse_order || m.take == 0 {
        return Vec::new();
    }
    let pile = piles.get(m.from);
    if pile.kind != PileKind::Waste || pile.fan_top_n <= 1 || pile.fan_dx == 0.0 {
        return Vec::new();
    }
    let take = m.take as usize;
    if pile.cards.len() <= take {
        return Vec::new();
    }
    let after_len = pile.cards.len() - take;
    (0..after_len)
        .map(|idx| {
            let (x, y) = pile.position_for(idx);
            (idx, pile.cards[idx], x, y)
        })
        .collect()
}

pub(super) fn queue_source_waste_reflow_animations(
    animations: &mut Vec<CardAnim>,
    sources: &[(usize, Card, f64, f64)],
    piles: &PileSet,
    m: &Move,
    now: Instant,
) {
    if sources.is_empty() {
        return;
    }
    let pile = piles.get(m.from);
    let mut shifted = Vec::new();
    for &(idx, card, src_x, src_y) in sources {
        if idx >= pile.cards.len() || pile.cards[idx] != card {
            continue;
        }
        let (dst_x, dst_y) = pile.position_for(idx);
        if (dst_x - src_x).abs() < 0.5 && (dst_y - src_y).abs() < 0.5 {
            continue;
        }
        shifted.push((idx, card, src_x, src_y, dst_x, dst_y));
    }
    let Some(hide_from) = shifted.iter().map(|(idx, ..)| *idx).min() else {
        return;
    };
    for (_idx, card, src_x, src_y, dst_x, dst_y) in shifted {
        animations.push(CardAnim {
            card,
            src_x,
            src_y,
            dst_x,
            dst_y,
            card_w: pile.card_w,
            card_h: pile.card_h,
            start_at: now,
            duration: Duration::from_millis(160),
            dst_pile: m.from,
            dst_hide_from: hide_from,
            flip: false,
            hold_before_start: false,
            late_appear_at: None,
        });
    }
}

pub(super) fn queue_klondike_draw_animations(
    animations: &mut Vec<CardAnim>,
    move_sources: &[(Card, f64, f64)],
    compact_sources: &[(usize, Card, f64, f64)],
    piles: &PileSet,
    m: &Move,
    now: Instant,
) {
    let waste = piles.get(m.to);
    let take = m.take as usize;
    if waste.cards.len() < take {
        return;
    }
    let draw_start = waste.cards.len() - take;

    // For each pre-existing fan card, compute its post position and
    // keep only the ones that actually move. Animating non-movers to
    // origin (the old behavior) made take<3 draws look like the fan
    // collapsed and then popped back. The remaining pre-fan cards
    // slide to their new fan slot.
    let mut shifted: Vec<(usize, Card, f64, f64, f64, f64)> = Vec::new();
    for &(idx, card, src_x, src_y) in compact_sources {
        if idx >= waste.cards.len() || waste.cards[idx] != card {
            continue;
        }
        let (dst_x, dst_y) = waste.position_for(idx);
        if (src_x - dst_x).abs() < 0.5 && (src_y - dst_y).abs() < 0.5 {
            continue;
        }
        shifted.push((idx, card, src_x, src_y, dst_x, dst_y));
    }
    let hide_from = shifted
        .iter()
        .map(|(idx, ..)| *idx)
        .min()
        .unwrap_or(draw_start);

    let compact_duration = Duration::from_millis(140);
    for &(_, card, src_x, src_y, dst_x, dst_y) in &shifted {
        animations.push(CardAnim {
            card,
            src_x,
            src_y,
            dst_x,
            dst_y,
            card_w: waste.card_w,
            card_h: waste.card_h,
            start_at: now,
            duration: compact_duration,
            dst_pile: m.to,
            dst_hide_from: hide_from,
            flip: false,
            hold_before_start: false,
            late_appear_at: None,
        });
    }

    // Only push the deal back behind the compaction when there IS
    // a shift. No-shift draws (empty waste, or fan already fits)
    // would otherwise render the just-applied waste cards unhidden
    // (no anim is `should_paint_now` yet) for 140 ms before they
    // get hidden and re-flown — a visible flash.
    let deal_start = if shifted.is_empty() {
        now
    } else {
        now + compact_duration
    };
    let deal_duration = Duration::from_millis(240);
    // Draw-3 must read as three distinct deals into slots 0, 1, 2.
    // Starting the next card only after the previous one lands keeps
    // the animated card and final static card in visual lockstep.
    let deal_stagger = deal_duration;
    for dst_offset in 0..take {
        let Some((_, src_x, src_y)) = move_sources.get(dst_offset).copied() else {
            continue;
        };
        let dst_idx = draw_start + dst_offset;
        let card = waste.cards[dst_idx];
        let (dst_x, dst_y) = waste.position_for(dst_idx);
        animations.push(CardAnim {
            card,
            src_x,
            src_y,
            dst_x,
            dst_y,
            card_w: waste.card_w,
            card_h: waste.card_h,
            start_at: deal_start + deal_stagger * dst_offset as u32,
            duration: deal_duration,
            dst_pile: m.to,
            dst_hide_from: dst_idx,
            flip: true,
            hold_before_start: false,
            late_appear_at: None,
        });
    }
}

pub(super) fn queue_recorded_move_animations(
    animations: &mut Vec<CardAnim>,
    records: &[AppliedMoveRecord],
    now: Instant,
    animate_user_move: bool,
) {
    let move_duration = Duration::from_millis(480);
    let swap_duration = Duration::from_millis(220);
    let flip_duration = Duration::from_millis(260);
    let auto_followup_pause = Duration::from_millis(120);
    let mut cursor = now;
    let mut previous_move: Option<Move> = None;
    let mut prev_anim_end: Option<Instant> = None;
    let mut deferred_source_flips: Vec<&AppliedMoveRecord> = Vec::new();

    for (record_idx, record) in records.iter().enumerate() {
        let next_is_auto = records.get(record_idx + 1).is_some_and(|next| next.is_auto);
        let should_animate_transfer = animate_user_move || record.is_auto;
        let mut transfer_duration = Duration::ZERO;

        if should_animate_transfer {
            if let Some(sources) = snapshot_swap_sources(&record.before, &record.m) {
                queue_swap_animations_from_snapshot(
                    animations,
                    &sources,
                    &record.after,
                    &record.m,
                    cursor,
                );
                transfer_duration = swap_duration;
            } else {
                let sources = snapshot_move_sources(&record.before, &record.m);
                if !sources.is_empty() {
                    // Hold the source pose only when this auto follow-up
                    // is consuming the previous move's destination — e.g.
                    // a Spider K-A run collapse fired by clicking an Ace
                    // onto a K-down-to-2. The just-landed top of the run
                    // appears *after* the user's flying animation ends so
                    // we don't double-draw with it; the rest of the run
                    // appears immediately to cover the static cascade
                    // that the session has already emptied.
                    let needs_hold =
                        previous_move.is_some_and(|prev| prev.to == record.m.from) && cursor > now;
                    let late_hold = if needs_hold {
                        previous_move
                            .map(|prev| prev.take as usize)
                            .zip(prev_anim_end)
                    } else {
                        None
                    };
                    queue_move_animations_from_snapshot(
                        animations,
                        &sources,
                        &record.after,
                        &record.m,
                        cursor,
                        Duration::ZERO,
                        needs_hold,
                        late_hold,
                    );
                    transfer_duration = move_duration;
                }
            }
        }

        let after_transfer = cursor + transfer_duration;
        let source_reflow = snapshot_source_waste_reflow(&record.before, &record.m);
        queue_source_waste_reflow_animations(
            animations,
            &source_reflow,
            &record.after,
            &record.m,
            cursor,
        );

        let mut flip_phase = false;
        if next_is_auto && record.m.flip_source_after {
            deferred_source_flips.push(record);
        } else {
            flip_phase = !deferred_source_flips.is_empty() || record.m.flip_source_after;
            for deferred in deferred_source_flips.drain(..) {
                queue_source_flip_animation_from_piles(
                    animations,
                    &deferred.after,
                    &deferred.m,
                    after_transfer,
                );
            }
            queue_source_flip_animation_from_piles(
                animations,
                &record.after,
                &record.m,
                after_transfer,
            );
        }

        prev_anim_end = Some(after_transfer);
        cursor = if flip_phase {
            after_transfer + flip_duration
        } else {
            after_transfer
        };
        if next_is_auto {
            cursor += auto_followup_pause;
        }
        previous_move = Some(record.m);
    }
}
