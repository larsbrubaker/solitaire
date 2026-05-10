//! Procedural face renderer for a single playing card.

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::text::Font;

use crate::cards::{Card, CardColor};
use crate::consts::CARD_CORNER_R;

use super::{CARD_BLACK, CARD_BORDER, CARD_FACE_BG, CARD_RED};

/// Paint a face-up card at the given Y-up rect.
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
    ctx.set_fill_color(CARD_FACE_BG);
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.fill();
    // Outline.
    ctx.set_stroke_color(CARD_BORDER);
    ctx.set_line_width(1.5);
    ctx.rounded_rect(x, y, w, h, CARD_CORNER_R);
    ctx.stroke();

    let color = match card.suit.color() {
        CardColor::Red => CARD_RED,
        CardColor::Black => CARD_BLACK,
    };

    let label = card.rank.label();
    let suit_glyph = card.suit.glyph().to_string();

    // ── Top-left corner: rank above, suit below ───────────────────────────
    ctx.set_fill_color(color);
    ctx.set_font(font.clone());
    ctx.set_font_size(20.0);
    let pad = 8.0;
    // y in Y-up is bottom — so top-of-card is y + h.
    // Baseline for rank label sits ~24px below top edge.
    let rank_baseline_y = y + h - 24.0;
    ctx.fill_text(label, x + pad, rank_baseline_y);
    ctx.set_font_size(18.0);
    let suit_baseline_y = y + h - 44.0;
    ctx.fill_text(&suit_glyph, x + pad, suit_baseline_y);

    // ── Bottom-right corner (rotated text — fake by mirroring position) ──
    // Without a per-glyph rotation primitive yet, mirror-position the
    // corner pair so it reads top-down from the bottom-right.
    ctx.set_font_size(20.0);
    let metric = ctx.measure_text(label);
    let label_w = metric.map(|m| m.width).unwrap_or(0.0);
    let br_label_x = x + w - pad - label_w;
    let br_label_baseline_y = y + 12.0;
    ctx.fill_text(label, br_label_x, br_label_baseline_y);
    ctx.set_font_size(18.0);
    let metric_s = ctx.measure_text(&suit_glyph);
    let suit_w = metric_s.map(|m| m.width).unwrap_or(0.0);
    let br_suit_x = x + w - pad - suit_w;
    let br_suit_baseline_y = y + 32.0;
    ctx.fill_text(&suit_glyph, br_suit_x, br_suit_baseline_y);

    // ── Center suit glyph (large) ────────────────────────────────────────
    ctx.set_font_size(48.0);
    let metric_c = ctx.measure_text(&suit_glyph);
    let cw = metric_c.map(|m| m.width).unwrap_or(0.0);
    let cx = x + (w - cw) / 2.0;
    let cy = y + h / 2.0 - 16.0; // visual center adjustment
    ctx.fill_text(&suit_glyph, cx, cy);
}
