//! Overlay banners painted over the playfield — the "You Won!" win
//! card and Mom's Solitaire's "Select a King" prompt. Extracted from
//! `game_widget.rs` to keep that file under the 800-line limit.

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::geometry::Rect;
use agg_gui::text::Font;

pub(super) fn paint_win_banner(ctx: &mut dyn DrawCtx, font: &Arc<Font>, rect: Rect) {
    use agg_gui::color::Color;
    let bg = Color::from_rgba8(0x10, 0x10, 0x10, 0xc8);
    let fg = Color::from_rgb8(0xff, 0xd7, 0x00);
    let pad = 30.0;
    let label = "You Won!";
    ctx.set_font(font.clone());
    ctx.set_font_size(56.0);
    let m = ctx.measure_text(label);
    let lw = m.map(|t| t.width).unwrap_or(280.0);
    let bw = lw + pad * 2.0;
    let bh = 100.0;
    let bx = rect.x + (rect.width - bw) / 2.0;
    let by = rect.y + (rect.height - bh) / 2.0;
    ctx.begin_path();
    ctx.rounded_rect(bx, by, bw, bh, 14.0);
    ctx.set_fill_color(bg);
    ctx.fill();
    ctx.set_fill_color(fg);
    ctx.fill_text(label, bx + pad, by + (bh - 56.0) / 2.0);
}

/// Status banner for Mom's Solitaire's "select a King for the empty
/// slot" prompt. Painted near the top of the playfield, similar in
/// style to the C# original's instruction banner.
pub(super) fn paint_moms_prompt(ctx: &mut dyn DrawCtx, font: &Arc<Font>, rect: Rect, label: &str) {
    use agg_gui::color::Color;
    let bg = Color::from_rgba8(0xf8, 0x89, 0x78, 0xf0);
    let border = Color::from_rgb8(0x20, 0x20, 0x20);
    let fg = Color::from_rgb8(0x10, 0x10, 0x10);
    let pad_x = 18.0;
    let bh = 32.0;
    let font_size = 16.0;
    ctx.set_font(font.clone());
    ctx.set_font_size(font_size);
    let m = ctx.measure_text(label);
    let lw = m.map(|t| t.width).unwrap_or(240.0);
    let bw = lw + pad_x * 2.0;
    let bx = rect.x + (rect.width - bw) / 2.0;
    // Y-up: place the banner near the TOP of the playfield rect.
    let by = rect.y + rect.height - bh - 8.0;
    ctx.begin_path();
    ctx.rounded_rect(bx, by, bw, bh, 6.0);
    ctx.set_fill_color(bg);
    ctx.fill();
    ctx.begin_path();
    ctx.rounded_rect(bx, by, bw, bh, 6.0);
    ctx.set_stroke_color(border);
    ctx.set_line_width(1.5);
    ctx.stroke();
    ctx.set_fill_color(fg);
    if let Some(m) = ctx.measure_text(label) {
        let baseline = by + m.centered_baseline_y(bh);
        ctx.fill_text(label, bx + pad_x, baseline);
    }
}
