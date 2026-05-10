//! Pile painter — draws all visible cards in a pile, optionally hiding a
//! tail of cards (used while dragging — the dragged stack paints elsewhere).

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::text::Font;

use crate::consts::{CARD_CORNER_R, CARD_H, CARD_W};
use crate::piles::Pile;

use super::{paint_card_back, paint_card_face, SLOT_BORDER};

/// Paint an empty-slot placeholder (a dashed rounded rect outline).
pub fn paint_empty_slot(ctx: &mut dyn DrawCtx, x: f64, y: f64) {
    ctx.set_stroke_color(SLOT_BORDER);
    ctx.set_line_width(2.0);
    ctx.rounded_rect(x, y, CARD_W, CARD_H, CARD_CORNER_R);
    ctx.stroke();
}

/// Paint every visible card in `pile`. If `hide_from` is `Some(idx)`,
/// cards `idx..` are NOT painted (used when those cards are being
/// dragged elsewhere).
pub fn paint_pile(ctx: &mut dyn DrawCtx, pile: &Pile, hide_from: Option<usize>, font: &Arc<Font>) {
    if pile.is_empty() {
        let (x, y, _, _) = pile.empty_slot_rect();
        paint_empty_slot(ctx, x, y);
        return;
    }

    let stop = hide_from.unwrap_or(pile.cards.len()).min(pile.cards.len());
    if stop == 0 {
        // All cards hidden — paint slot underneath so destination
        // tableau columns still show their slot outline.
        let (x, y, _, _) = pile.empty_slot_rect();
        paint_empty_slot(ctx, x, y);
        return;
    }

    for idx in 0..stop {
        let (x, y, w, h) = pile.card_rect(idx);
        let card = &pile.cards[idx];
        if card.face_up {
            paint_card_face(ctx, card, x, y, w, h, font);
        } else {
            paint_card_back(ctx, x, y, w, h);
        }
    }
}
