//! Procedural back-of-card renderer.

use agg_gui::draw_ctx::DrawCtx;

use crate::consts::CARD_CORNER_R;

use super::{CARD_BACK_BG, CARD_BACK_PATTERN, CARD_BORDER};

/// Paint a face-down card at the given Y-up rect. Uses a simple
/// diamond-grid pattern over a navy background.
pub fn paint_card_back(ctx: &mut dyn DrawCtx, x: f64, y: f64, w: f64, h: f64) {
    // Background.
    ctx.set_fill_color(CARD_BACK_BG);
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.fill();

    // Pattern: small diamonds laid out on a regular grid, clipped to the
    // card's rounded rect by drawing the rect again as a clip.
    ctx.save();
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.clip_rect(x, y, w, h);

    ctx.set_fill_color(CARD_BACK_PATTERN);
    let step = 14.0;
    let r = 4.0;
    let mut row_y = y + step;
    let mut row_idx: i32 = 0;
    while row_y < y + h - r {
        let offset = if row_idx % 2 == 0 { 0.0 } else { step / 2.0 };
        let mut cx = x + step / 2.0 + offset;
        while cx < x + w - r {
            // Diamond drawn as a square rotated 45° — easier to use ellipse
            // approximation by drawing a filled circle (close enough at
            // this scale).
            ctx.circle(cx, row_y, r);
            ctx.fill();
            cx += step;
        }
        row_y += step;
        row_idx += 1;
    }
    ctx.restore();

    // Outline.
    ctx.set_stroke_color(CARD_BORDER);
    ctx.set_line_width(1.5);
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.stroke();
}
