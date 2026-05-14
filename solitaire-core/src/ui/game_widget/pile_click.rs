//! Pile-click handler for `GameWidget`. Split out so the host file
//! stays under the 800-line limit. Lives in its own `impl GameWidget`
//! block.

use crate::cards::Card;
use crate::piles::{HitResult, PileId};

use crate::ui::animation::{build_stripe_texture, deck_thickness_for, CardAnim, DeckFlipAnim};

use super::animations;
use super::GameWidget;

impl GameWidget {
    pub(super) fn try_pile_click(&mut self, vx: f64, vy: f64) -> bool {
        let mut model = self.model.borrow_mut();
        let Some(session) = model.session.as_mut() else {
            return false;
        };
        let piles = session.piles();
        let Some(hit) = piles.hit_test(vx, vy) else {
            return false;
        };
        let pile_id = match hit {
            HitResult::Card { pile, .. } => pile,
            HitResult::EmptySlot { pile } => pile,
        };
        let moves = session.on_pile_click(pile_id);
        if moves.is_empty() {
            return false;
        }
        // Detect Klondike's waste→stock recycle: a single move whose
        // SOURCE is a different pile from the one we clicked, that
        // takes the whole pile and reverses it. That's our cue to
        // animate the ENTIRE waste flipping as one 3-D deck box,
        // rather than as N per-card flights.
        let is_recycle = moves.len() == 1
            && moves[0].from != pile_id
            && moves[0].reverse_order
            && moves[0].flip_moved;
        let single_move_animation = if !is_recycle && moves.len() == 1 {
            let m = moves[0];
            let move_sources = animations::snapshot_move_sources(piles, &m);
            let compact_sources = if animations::is_klondike_stock_to_waste_draw(piles, &m) {
                animations::snapshot_waste_fan_compaction(piles, m.to)
            } else {
                Vec::new()
            };
            Some((m, move_sources, compact_sources))
        } else {
            None
        };

        // Capture the source origin (top of the clicked pile) BEFORE
        // applying the batch — once we apply, the stock's top card
        // has moved and we can't read its pre-move position.
        let src_pile = piles.get(pile_id);
        let src_x = src_pile.origin_x;
        let src_y = src_pile.origin_y;
        let dst_pile_ids: Vec<PileId> = moves.iter().map(|m| m.to).collect();

        // Recycle path: snapshot the waste's pre-move state so the
        // animation knows the card count + top card before the
        // session mutates them away.
        let recycle_setup: Option<(PileId, Card, u32, f64, f64, f64, f64)> = if is_recycle {
            let from_id = moves[0].from;
            let waste = piles.get(from_id);
            let top = waste.top().copied();
            let count = waste.cards.len() as u32;
            // Top of waste = visible center of waste pile.
            let (wx, wy) = if !waste.cards.is_empty() {
                waste.position_for(waste.cards.len() - 1)
            } else {
                (waste.origin_x, waste.origin_y)
            };
            top.map(|t| (from_id, t, count, wx, wy, waste.card_w, waste.card_h))
        } else {
            None
        };

        let Some(records) = session.try_apply_batch_recording(moves) else {
            return false;
        };

        let now = web_time::Instant::now();
        if let Some((_from_id, top_card, card_count, wx, wy, card_w, card_h)) = recycle_setup {
            let dst_pile = session.piles().get(dst_pile_ids[0]);
            let (sx, sy) = if !dst_pile.cards.is_empty() {
                dst_pile.position_for(dst_pile.cards.len() - 1)
            } else {
                (dst_pile.origin_x, dst_pile.origin_y)
            };
            let thickness = deck_thickness_for(card_count, card_w);
            let (side_tex, side_w, side_h) = build_stripe_texture(card_count);
            let dst_hide_from = if dst_pile.cards.is_empty() {
                0
            } else {
                dst_pile.cards.len() - card_count.min(dst_pile.cards.len() as u32) as usize
            };
            self.deck_animations.push(DeckFlipAnim {
                top_card,
                card_count,
                card_w,
                card_h,
                thickness,
                src_center_x: wx + card_w / 2.0,
                src_center_y: wy + card_h / 2.0,
                dst_center_x: sx + card_w / 2.0,
                dst_center_y: sy + card_h / 2.0,
                start_at: now,
                duration: std::time::Duration::from_millis(450),
                dst_pile: dst_pile_ids[0],
                dst_hide_from,
                side_texture: side_tex,
                side_tex_w: side_w,
                side_tex_h: side_h,
            });
        } else if let Some((m, sources, compact_sources)) = single_move_animation {
            if animations::is_klondike_stock_to_waste_draw(session.piles(), &m) {
                animations::queue_klondike_draw_animations(
                    &mut self.animations,
                    &sources,
                    &compact_sources,
                    session.piles(),
                    &m,
                    now,
                );
            } else {
                animations::queue_move_animations_from_snapshot(
                    &mut self.animations,
                    &sources,
                    session.piles(),
                    &m,
                    now,
                    std::time::Duration::from_millis(70),
                    false,
                    None,
                );
            }
        } else {
            // Regular fly-and-flip per dispensed card.
            let per_card_dur = std::time::Duration::from_millis(220);
            let stagger = std::time::Duration::from_millis(40);
            for (i, dst_id) in dst_pile_ids.iter().enumerate() {
                let dst_pile = session.piles().get(*dst_id);
                if dst_pile.cards.is_empty() {
                    continue;
                }
                let new_idx = dst_pile.cards.len() - 1;
                let card = dst_pile.cards[new_idx];
                let (dx, dy) = dst_pile.position_for(new_idx);
                self.animations.push(CardAnim {
                    card,
                    src_x,
                    src_y,
                    dst_x: dx,
                    dst_y: dy,
                    card_w: dst_pile.card_w,
                    card_h: dst_pile.card_h,
                    start_at: now + stagger * i as u32,
                    duration: per_card_dur,
                    dst_pile: *dst_id,
                    dst_hide_from: new_idx,
                    flip: true,
                    hold_before_start: false,
                    late_appear_at: None,
                });
            }
        }
        let batch_len = dst_pile_ids.len();
        if records.len() > batch_len {
            animations::queue_recorded_move_animations(
                &mut self.animations,
                &records[batch_len..],
                now,
                true,
            );
        }
        agg_gui::animation::request_draw();
        true
    }
}
