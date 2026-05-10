//! Pile painter — blits cached card sprites from the `CardSpriteAtlas`
//! via `DrawCtx::draw_image_rgba_arc`. The wgpu backend keys textures
//! by `Arc::as_ptr`, so the 53 unique sprites upload ONCE and every
//! per-frame draw is a textured quad.
//!
//! ## 1:1 pixel-perfect blits
//!
//! The atlas pre-rasterises sprites at exactly `(CARD_W × target,
//! CARD_H × target)` device pixels, where `target = playfield_scale ×
//! device_scale`. By paint time, the framework's outer scale (device
//! scale) plus this widget's `ctx.scale(playfield_scale, _)` make the
//! current CTM's `sx`/`sy` equal `target`. If we naively asked the
//! GPU to draw at logical `CARD_W × CARD_H`, the destination physical
//! size would be `CARD_W × target` — a non-integer in general — while
//! the texture is `round(CARD_W × target)` pixels. The mismatch makes
//! the GPU stretch the source by a sub-pixel amount under NEAREST
//! sampling, which shows up as duplicate-column "sampling lines" in
//! the otherwise-flat parts of the card art.
//!
//! Fix: read the current CTM, compute logical destination sizes that
//! map to EXACTLY `atlas.px_w / atlas.px_h` device pixels, and snap
//! each card's origin to integer device pixels so adjacent cards
//! don't smear across sub-pixel boundaries either.

use agg_gui::draw_ctx::DrawCtx;

use crate::consts::{CARD_CORNER_R, CARD_H, CARD_W};
use crate::piles::Pile;

use super::atlas::CardSpriteAtlas;
use super::SLOT_BORDER;

/// Paint an empty-slot placeholder. Cheap (one rounded-rect stroke) so
/// no atlas entry is needed.
pub fn paint_empty_slot(ctx: &mut dyn DrawCtx, x: f64, y: f64) {
    ctx.begin_path();
    ctx.rounded_rect(x, y, CARD_W, CARD_H, CARD_CORNER_R);
    ctx.set_stroke_color(SLOT_BORDER);
    ctx.set_line_width(2.0);
    ctx.stroke();
}

/// Logical destination rect that maps `(atlas.px_w, atlas.px_h)`
/// physical pixels onto integer device-pixel boundaries at logical
/// origin `(lx, ly)`. Returns `(snapped_lx, snapped_ly, dst_w_log,
/// dst_h_log)`.
fn snap_blit_rect(
    ctx: &dyn DrawCtx,
    atlas: &CardSpriteAtlas,
    lx: f64,
    ly: f64,
) -> (f64, f64, f64, f64) {
    let t = ctx.transform();
    let sx = t.sx.abs().max(1e-9);
    let sy = t.sy.abs().max(1e-9);
    // Sign of sy can be negative (Y-up flip baked into the framework's
    // outer transform); preserve it so we don't accidentally Y-flip the
    // sprite when computing the snap.
    let sy_signed = if t.sy >= 0.0 { sy } else { -sy };
    let phys_x = (lx * t.sx + t.tx).round();
    let phys_y = (ly * t.sy + t.ty).round();
    let snapped_lx = (phys_x - t.tx) / t.sx;
    let snapped_ly = (phys_y - t.ty) / t.sy;
    let dst_w_log = atlas.px_w as f64 / sx;
    let dst_h_log = atlas.px_h as f64 / sy_signed.abs();
    (snapped_lx, snapped_ly, dst_w_log, dst_h_log)
}

/// Paint every visible card in `pile` by blitting cached sprites.
/// `hide_from = Some(idx)` suppresses cards `idx..` (used during drag).
pub fn paint_pile(
    ctx: &mut dyn DrawCtx,
    pile: &Pile,
    hide_from: Option<usize>,
    atlas: &CardSpriteAtlas,
) {
    if pile.is_empty() {
        let (x, y, _, _) = pile.empty_slot_rect();
        paint_empty_slot(ctx, x, y);
        return;
    }

    let stop = hide_from.unwrap_or(pile.cards.len()).min(pile.cards.len());
    if stop == 0 {
        let (x, y, _, _) = pile.empty_slot_rect();
        paint_empty_slot(ctx, x, y);
        return;
    }

    for idx in 0..stop {
        let (lx, ly) = pile.position_for(idx);
        let (sx, sy, w, h) = snap_blit_rect(ctx, atlas, lx, ly);
        let card = &pile.cards[idx];
        let sprite = if card.face_up {
            atlas.face(card.suit, card.rank)
        } else {
            atlas.back()
        };
        ctx.draw_image_rgba_arc(&sprite, atlas.px_w, atlas.px_h, sx, sy, w, h);
    }
}

/// Paint a single card at an arbitrary (x, y) — used by the GameWidget
/// drag overlay where cards float at the cursor.
pub fn paint_card_at(
    ctx: &mut dyn DrawCtx,
    card: &crate::cards::Card,
    x: f64,
    y: f64,
    atlas: &CardSpriteAtlas,
) {
    let (sx, sy, w, h) = snap_blit_rect(ctx, atlas, x, y);
    let sprite = if card.face_up {
        atlas.face(card.suit, card.rank)
    } else {
        atlas.back()
    };
    ctx.draw_image_rgba_arc(&sprite, atlas.px_w, atlas.px_h, sx, sy, w, h);
}
