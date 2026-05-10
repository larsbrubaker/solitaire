//! Procedural back-of-card renderer.
//!
//! Kept deliberately cheap — under 5 DrawCtx primitives — because a
//! Klondike deal renders up to 24 face-down cards every frame and the
//! software AGG path scales linearly with primitive count.

use agg_gui::draw_ctx::DrawCtx;

use crate::consts::CARD_CORNER_R;

use super::{CARD_BACK_BG, CARD_BACK_PATTERN, CARD_BORDER};

pub fn paint_card_back(ctx: &mut dyn DrawCtx, x: f64, y: f64, w: f64, h: f64) {
    // Solid back.
    ctx.set_fill_color(CARD_BACK_BG);
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.fill();

    // Inset highlight rectangle — gives the back a "framed" feel without
    // a per-pixel pattern.
    let inset = 7.0;
    ctx.set_stroke_color(CARD_BACK_PATTERN);
    ctx.set_line_width(2.0);
    ctx.rounded_rect(x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset, CARD_CORNER_R - 2.0);
    ctx.stroke();

    // Outline.
    ctx.set_stroke_color(CARD_BORDER);
    ctx.set_line_width(1.5);
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.stroke();
}
