//! Pile painter — blits cached card sprites from the `CardSpriteAtlas`
//! via `DrawCtx::draw_image_rgba_arc`. The wgpu backend keys textures
//! by `Arc::as_ptr`, so the 53 unique sprites upload ONCE and every
//! per-frame draw is a textured quad.

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
        let (x, y, w, h) = pile.card_rect(idx);
        let card = &pile.cards[idx];
        let sprite = if card.face_up {
            atlas.face(card.suit, card.rank)
        } else {
            atlas.back()
        };
        ctx.draw_image_rgba_arc(&sprite, atlas.px_w, atlas.px_h, x, y, w, h);
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
    let sprite = if card.face_up {
        atlas.face(card.suit, card.rank)
    } else {
        atlas.back()
    };
    ctx.draw_image_rgba_arc(&sprite, atlas.px_w, atlas.px_h, x, y, CARD_W, CARD_H);
}
