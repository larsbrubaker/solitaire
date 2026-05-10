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

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;

use crate::cards::Rank;
use crate::consts::CARD_CORNER_R;
use crate::piles::Pile;

use super::atlas::CardSpriteAtlas;
use super::{FELT_GREEN_DARK, SLOT_BORDER};

/// Mom's Solitaire gap fill — a soft inner panel inside the slot
/// border so the Ace cells read clearly as drop targets, distinct
/// from the felt and from a Klondike-style empty pile placeholder.
const MOMS_GAP_FILL: Color = FELT_GREEN_DARK;

/// Paint an empty-slot placeholder. Cheap (one rounded-rect stroke) so
/// no atlas entry is needed. Caller passes the slot's per-pile
/// dimensions so Mom's Solitaire's smaller cells render correctly.
pub fn paint_empty_slot(ctx: &mut dyn DrawCtx, x: f64, y: f64, w: f64, h: f64) {
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.set_stroke_color(SLOT_BORDER);
    ctx.set_line_width(2.0);
    ctx.stroke();
}

/// Paint a Mom's-Solitaire gap (Ace cell rendered as a hole rather
/// than as a card). Filled with a slightly darker green and an
/// outlined border so the player can see at a glance which cells
/// are drop targets.
fn paint_gap_slot(ctx: &mut dyn DrawCtx, x: f64, y: f64, w: f64, h: f64) {
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.set_fill_color(MOMS_GAP_FILL);
    ctx.fill();
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.set_stroke_color(SLOT_BORDER);
    ctx.set_line_width(2.0);
    ctx.stroke();
}

/// Logical destination rect for a single card blit, with the origin
/// snapped to integer device pixels so adjacent cards don't smear at
/// sub-pixel boundaries. Returns `(snapped_lx, snapped_ly, dst_w_log,
/// dst_h_log)`.
///
/// Standard variants (Klondike / FreeCell / Spider) pass the atlas's
/// per-card pixel size in `(card_w_log, card_h_log)` translated through
/// the current scale — the destination then matches `atlas.px_w / px_h`
/// exactly and NEAREST sampling stays 1:1. Mom's Solitaire passes a
/// smaller card_w/card_h to render its 13×4 grid; the GPU
/// downsamples (slight quality loss vs. the standard variants, but
/// the cells need to fit in the playfield).
fn snap_blit_rect(
    ctx: &dyn DrawCtx,
    lx: f64,
    ly: f64,
    card_w_log: f64,
    card_h_log: f64,
) -> (f64, f64, f64, f64) {
    let t = ctx.transform();
    let phys_x = (lx * t.sx + t.tx).round();
    let phys_y = (ly * t.sy + t.ty).round();
    let snapped_lx = (phys_x - t.tx) / t.sx;
    let snapped_ly = (phys_y - t.ty) / t.sy;
    (snapped_lx, snapped_ly, card_w_log, card_h_log)
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
        let (x, y, w, h) = pile.empty_slot_rect();
        paint_empty_slot(ctx, x, y, w, h);
        return;
    }

    let stop = hide_from.unwrap_or(pile.cards.len()).min(pile.cards.len());
    if stop == 0 {
        let (x, y, w, h) = pile.empty_slot_rect();
        paint_empty_slot(ctx, x, y, w, h);
        return;
    }

    // Mom's Solitaire: a single-card pile whose top is an Ace renders
    // as a gap rather than as the Ace card. Drop-target visual.
    if pile.render_ace_as_gap && pile.cards.len() == 1 && pile.cards[0].rank == Rank::Ace {
        let (x, y) = pile.position_for(0);
        paint_gap_slot(ctx, x, y, pile.card_w, pile.card_h);
        return;
    }

    for idx in 0..stop {
        let (lx, ly) = pile.position_for(idx);
        let (sx, sy, w, h) = snap_blit_rect(ctx, lx, ly, pile.card_w, pile.card_h);
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
/// drag overlay where cards float at the cursor. Caller passes the
/// destination card size so dragged cards from a Mom's-Solitaire-sized
/// pile keep their smaller dimensions while floating.
pub fn paint_card_at(
    ctx: &mut dyn DrawCtx,
    card: &crate::cards::Card,
    x: f64,
    y: f64,
    card_w: f64,
    card_h: f64,
    atlas: &CardSpriteAtlas,
) {
    let (sx, sy, w, h) = snap_blit_rect(ctx, x, y, card_w, card_h);
    let sprite = if card.face_up {
        atlas.face(card.suit, card.rank)
    } else {
        atlas.back()
    };
    ctx.draw_image_rgba_arc(&sprite, atlas.px_w, atlas.px_h, sx, sy, w, h);
}
