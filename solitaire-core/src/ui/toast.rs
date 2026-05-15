//! Transient status banner shared by Title and Game screens. The
//! caller computes its own placement rect; this module just renders
//! the pill + text with an automatic fade-out near the end of the
//! toast's lifetime. Pairs with `AppModel::show_toast` /
//! `AppModel::tick_toast` and the `TOAST_LIFETIME` constant — clears
//! itself on its own once the lifetime elapses.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::text::Font;
use web_time::Instant;

use super::app_model::TOAST_LIFETIME;

const TOAST_BG: Color = Color::from_rgba8(0x10, 0x10, 0x10, 0xc0);
const TOAST_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

/// Final stretch of `TOAST_LIFETIME` during which the toast fades
/// from full opacity down to zero.
const FADE_DURATION: std::time::Duration = std::time::Duration::from_millis(900);

/// Render the toast pill horizontally centred inside `[origin_x,
/// origin_x + width]` with its bottom-left at `(x_centered,
/// origin_y)`. No-op once the toast has fully faded.
pub fn paint_toast(
    ctx: &mut dyn DrawCtx,
    font: &Arc<Font>,
    origin_x: f64,
    origin_y: f64,
    width: f64,
    msg: &str,
    started: Instant,
) {
    let Some(alpha) = current_alpha(started) else {
        return;
    };
    let pad = 16.0;
    ctx.set_font(font.clone());
    ctx.set_font_size(20.0);
    let m = ctx.measure_text(msg);
    let tw = m.map(|t| t.width).unwrap_or(220.0) + pad * 2.0;
    let th = 40.0;
    let x = origin_x + (width - tw) / 2.0;
    let y = origin_y;
    ctx.set_global_alpha(alpha);
    ctx.begin_path();
    ctx.rounded_rect(x, y, tw, th, 8.0);
    ctx.set_fill_color(TOAST_BG);
    ctx.fill();
    ctx.set_fill_color(TOAST_TEXT);
    ctx.fill_text(msg, x + pad, y + (th - 20.0) / 2.0);
    ctx.set_global_alpha(1.0);
}

/// Alpha for the toast given how long ago it was raised. Returns
/// `None` once the toast has fully faded so callers can skip the
/// draw entirely.
fn current_alpha(started: Instant) -> Option<f64> {
    let elapsed = started.elapsed();
    if elapsed >= TOAST_LIFETIME {
        return None;
    }
    let fade_dur = FADE_DURATION.min(TOAST_LIFETIME);
    let fade_start = TOAST_LIFETIME.saturating_sub(fade_dur);
    if elapsed <= fade_start {
        Some(1.0)
    } else {
        let into_fade = (elapsed - fade_start).as_secs_f64();
        let fade_s = fade_dur.as_secs_f64().max(0.001);
        Some((1.0 - into_fade / fade_s).clamp(0.0, 1.0))
    }
}
