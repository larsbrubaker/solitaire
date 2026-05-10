//! Procedural face renderer for a single playing card.
//!
//! `begin_path()` is required before every shape — `rounded_rect` /
//! `rect` / `circle` ADD to the active path, and `fill` / `stroke`
//! rasterise everything accumulated so far. Skipping `begin_path` makes
//! frame time grow O(n²) with the number of cards on the board.

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::text::Font;

use crate::cards::{Card, CardColor};
use crate::consts::CARD_CORNER_R;

use super::{CARD_BLACK, CARD_BORDER, CARD_FACE_BG, CARD_RED};

pub fn paint_card_face(
    ctx: &mut dyn DrawCtx,
    card: &Card,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    font: &Arc<Font>,
) {
    // Background.
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.set_fill_color(CARD_FACE_BG);
    ctx.fill();

    // Outline.
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.set_stroke_color(CARD_BORDER);
    ctx.set_line_width(1.5);
    ctx.stroke();

    let color = match card.suit.color() {
        CardColor::Red => CARD_RED,
        CardColor::Black => CARD_BLACK,
    };

    let label = card.rank.label();
    let suit_glyph = card.suit.glyph().to_string();

    ctx.set_fill_color(color);
    ctx.set_font(font.clone());

    // Top-left corner pair.
    let pad = 8.0;
    ctx.set_font_size(20.0);
    ctx.fill_text(label, x + pad, y + h - 24.0);
    ctx.set_font_size(16.0);
    ctx.fill_text(&suit_glyph, x + pad, y + h - 44.0);

    // Center suit glyph (large).
    ctx.set_font_size(48.0);
    let metric_c = ctx.measure_text(&suit_glyph);
    let cw = metric_c.map(|m| m.width).unwrap_or(0.0);
    let cx = x + (w - cw) / 2.0;
    let cy = y + h / 2.0 - 16.0;
    ctx.fill_text(&suit_glyph, cx, cy);
}
