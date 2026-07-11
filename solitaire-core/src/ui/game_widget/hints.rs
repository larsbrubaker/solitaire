//! Spider Hint machinery for `GameWidget` — the one-shot ghost preview,
//! the attention pulse, and the fade-out tail on the yellow highlight
//! rects. Split out of `game_widget.rs` to keep that file under the
//! 800-line limit; lives in its own `impl GameWidget` block and reads
//! the parent's private fields via `super::`.

use agg_gui::draw_ctx::DrawCtx;

use crate::cards::Card;
use crate::render::paint_card_at;

use super::GameWidget;

/// Stroke a rounded yellow outline over a card-sized rect — used by the
/// Spider Hint overlay to mark the recommended source run, destination,
/// or stock pile when the recommended action is a stock deal. `alpha`
/// is the effective opacity for this frame; the rect is painted in
/// fully-opaque yellow modulated through `global_alpha` so the pulse
/// and fade-out animations both ride on a single knob.
fn stroke_hint_rect(ctx: &mut dyn DrawCtx, x: f64, y: f64, w: f64, h: f64, alpha: f64) {
    ctx.set_global_alpha(alpha.clamp(0.0, 1.0));
    ctx.begin_path();
    ctx.rounded_rect(x, y, w, h, crate::consts::CARD_CORNER_R);
    ctx.set_stroke_color(agg_gui::color::Color::from_rgb8(0xff, 0xd7, 0x00));
    ctx.set_line_width(4.0);
    ctx.stroke();
    ctx.set_global_alpha(1.0);
}

/// One-shot ghost preview kicked off by the Hint button — snapshots
/// the source cards plus their src + dst bottom-left positions, then
/// the widget interpolates the stack from src to dst and fades it out.
#[derive(Clone, Debug)]
pub(super) struct HintAnim {
    cards: Vec<Card>,
    src_x: f64,
    src_y: f64,
    dst_x: f64,
    dst_y: f64,
    card_w: f64,
    card_h: f64,
    /// Source pile's fan-step scale, so the ghost fans like the pile.
    fan_scale: f64,
    start_at: web_time::Instant,
    slide_dur: std::time::Duration,
    fade_dur: std::time::Duration,
}

impl HintAnim {
    /// Returns the cards' bottom-left for this frame and the alpha to
    /// paint them at. Once `done()` flips true the animation is dead.
    fn current(&self) -> (f64, f64, f64) {
        let el = self.start_at.elapsed().as_secs_f64().max(0.0);
        let slide_s = self.slide_dur.as_secs_f64();
        let fade_s = self.fade_dur.as_secs_f64();
        let slide_t = (el / slide_s).clamp(0.0, 1.0);
        // Ease-out cubic so the stack decelerates as it lands.
        let eased = 1.0 - (1.0 - slide_t).powi(3);
        let bx = self.src_x + (self.dst_x - self.src_x) * eased;
        let by = self.src_y + (self.dst_y - self.src_y) * eased;
        let fade_t = ((el - slide_s) / fade_s).clamp(0.0, 1.0);
        let alpha = 0.6 * (1.0 - fade_t);
        (bx, by, alpha)
    }

    pub(super) fn done(&self) -> bool {
        self.start_at.elapsed() >= self.slide_dur + self.fade_dur
    }
}

/// One-shot attention pulse for the hint highlight. Three-phase
/// envelope so the rect fades up to peak, fades back down to
/// near-invisible, then fades back up to the static resting
/// brightness — matching the alpha of `crate::render::HIGHLIGHT`
/// (~0.5) at the end so the hand-off to the steady-state rect
/// drawn after `done()` is seamless (no pop).
#[derive(Clone, Debug)]
pub(super) struct HintPulse {
    start_at: web_time::Instant,
    duration: std::time::Duration,
}

/// Smooth tail-off animation kicked off when the user touches the
/// board (mouse-down on the playfield) or commits a move/undo while
/// a hint highlight is still visible. Captures whatever alpha the
/// rect was painting at the moment of interruption so the fade
/// starts from the current brightness rather than snapping.
#[derive(Clone, Debug)]
pub(super) struct HintFadeOut {
    pub(super) start_at: web_time::Instant,
    pub(super) duration: std::time::Duration,
    starting_alpha: f64,
}

/// Effective alpha of the static `crate::render::HIGHLIGHT` colour —
/// the value the pulse must land on so the transition into the
/// steady-state rect is invisible.
const HINT_REST_ALPHA: f64 = 0x80 as f64 / 255.0;

impl HintPulse {
    fn alpha_factor(&self) -> Option<f64> {
        let el = self.start_at.elapsed().as_secs_f64();
        let dur = self.duration.as_secs_f64();
        if el >= dur {
            return None;
        }
        let t = (el / dur).clamp(0.0, 1.0);
        // Phase splits — picked so the up/down beat is the dominant
        // motion and the recovery is a quick settle.
        const UP_END: f64 = 0.35;
        const DOWN_END: f64 = 0.70;
        let alpha = if t < UP_END {
            // 0 → 1.
            t / UP_END
        } else if t < DOWN_END {
            // 1 → 0.
            1.0 - (t - UP_END) / (DOWN_END - UP_END)
        } else {
            // 0 → resting alpha.
            let u = (t - DOWN_END) / (1.0 - DOWN_END);
            HINT_REST_ALPHA * u
        };
        Some(alpha.clamp(0.0, 1.0))
    }

    pub(super) fn done(&self) -> bool {
        self.start_at.elapsed() >= self.duration
    }
}

impl GameWidget {
    /// Resolve the alpha the hint rect should paint at this frame.
    /// `None` means "no rect at all" (no displayed hint or fade
    /// completed). Layer priority is fade-out → pulse → resting alpha.
    fn current_hint_alpha(&self) -> Option<f64> {
        self.displayed_hint?;
        if let Some(fade) = self.hint_fade_out.as_ref() {
            let el = fade.start_at.elapsed().as_secs_f64();
            let dur = fade.duration.as_secs_f64().max(1e-6);
            if el >= fade.duration.as_secs_f64() {
                return None;
            }
            let t = (el / dur).clamp(0.0, 1.0);
            return Some(fade.starting_alpha * (1.0 - t));
        }
        if let Some(pulse) = self.hint_pulse.as_ref() {
            return Some(pulse.alpha_factor().unwrap_or(HINT_REST_ALPHA));
        }
        Some(HINT_REST_ALPHA)
    }

    /// Begin a fade-out from whatever alpha the rect was painting at
    /// just now. Idempotent — re-entry while a fade is already in
    /// flight is a no-op. Also clears `model.spider_hint` so other
    /// observers see the hint gone immediately; only the widget's
    /// `displayed_hint` lingers, just long enough for the fade.
    pub(super) fn start_hint_fade_out(&mut self) {
        if self.displayed_hint.is_none() || self.hint_fade_out.is_some() {
            return;
        }
        let starting_alpha = if let Some(pulse) = self.hint_pulse.as_ref() {
            pulse.alpha_factor().unwrap_or(HINT_REST_ALPHA)
        } else {
            HINT_REST_ALPHA
        };
        self.hint_fade_out = Some(HintFadeOut {
            start_at: web_time::Instant::now(),
            duration: std::time::Duration::from_millis(400),
            starting_alpha,
        });
        // Cancel the in-flight pulse + ghost slide so they don't
        // re-brighten the rect while we're trying to fade it down.
        self.hint_pulse = None;
        self.hint_anim = None;
        self.model.borrow_mut().spider_hint = None;
        agg_gui::animation::request_draw();
    }

    /// Paint the Spider Hint overlay: yellow outlines around the
    /// recommended source run and destination (or just the stock pile
    /// for a stock-deal hint). The companion ghost-card preview that
    /// animates source → destination on every Hint button press is
    /// painted by `paint_hint_animation`.
    pub(super) fn paint_spider_hint_overlay(&self, ctx: &mut dyn DrawCtx) {
        let model = self.model.borrow();
        let Some(hint) = self.displayed_hint else {
            return;
        };
        let Some(alpha) = self.current_hint_alpha() else {
            return;
        };
        let Some(session) = model.session.as_ref() else {
            return;
        };
        let piles = session.piles();
        match hint {
            crate::games::spider::SpiderHint::Move {
                from,
                start_idx,
                take,
                to,
            } => {
                let src = piles.get(from);
                let take = take as usize;
                if start_idx >= src.cards.len() || take == 0 {
                    return;
                }
                let end_idx = (start_idx + take - 1).min(src.cards.len() - 1);
                let (hx, hy) = src.position_for(start_idx);
                let (_tx, ty) = src.position_for(end_idx);
                let x = hx;
                let y = ty;
                let w = src.card_w;
                let h = hy + src.card_h - ty;
                stroke_hint_rect(ctx, x, y, w, h, alpha);
                let dst = piles.get(to);
                let (dx, dy, dw, dh) = if dst.is_empty() {
                    dst.empty_slot_rect()
                } else {
                    dst.card_rect(dst.cards.len() - 1)
                };
                stroke_hint_rect(ctx, dx, dy, dw, dh, alpha);
            }
            crate::games::spider::SpiderHint::StockDeal { stock } => {
                let pile = piles.get(stock);
                let (sx, sy, sw, sh) = if pile.is_empty() {
                    pile.empty_slot_rect()
                } else {
                    pile.card_rect(pile.cards.len() - 1)
                };
                stroke_hint_rect(ctx, sx, sy, sw, sh, alpha);
            }
        }
    }

    /// Detect a fresh Hint button press (via `AppModel::spider_hint_seq`
    /// bumping) and snapshot the source/destination positions for a
    /// new ghost preview animation. Called every paint; cheap when
    /// nothing changed.
    pub(super) fn tick_hint_animation(&mut self) {
        let model = self.model.borrow();
        let seq = model.spider_hint_seq;
        if seq == self.last_hint_seq {
            // Drop finished one-shots.
            if self.hint_anim.as_ref().is_some_and(HintAnim::done) {
                self.hint_anim = None;
            }
            if self.hint_pulse.as_ref().is_some_and(HintPulse::done) {
                self.hint_pulse = None;
            }
            // Model cleared the hint (move/undo) without bumping seq —
            // fade the rect out instead of letting it pop off.
            if model.spider_hint.is_none()
                && self.displayed_hint.is_some()
                && self.hint_fade_out.is_none()
            {
                drop(model);
                self.start_hint_fade_out();
                return;
            }
            // Retire a finished fade.
            if let Some(fade) = self.hint_fade_out.as_ref() {
                if fade.start_at.elapsed() >= fade.duration {
                    self.hint_fade_out = None;
                    self.displayed_hint = None;
                }
            }
            return;
        }
        self.last_hint_seq = seq;
        self.hint_anim = None;
        self.hint_pulse = None;
        self.hint_fade_out = None;
        self.displayed_hint = model.spider_hint;

        let Some(hint) = model.spider_hint else {
            return;
        };
        // Every fresh hint kicks off the attention pulse so the rect
        // visibly fades up and back down once. Move hints also get
        // the ghost-card preview; StockDeal hints rely on pulse +
        // toast alone (there's no useful flight path for "click the
        // stock pile").
        self.hint_pulse = Some(HintPulse {
            start_at: web_time::Instant::now(),
            duration: std::time::Duration::from_millis(1100),
        });
        let crate::games::spider::SpiderHint::Move {
            from,
            start_idx,
            take,
            to,
        } = hint
        else {
            agg_gui::animation::request_draw();
            return;
        };
        let Some(session) = model.session.as_ref() else {
            return;
        };
        let piles = session.piles();
        let src = piles.get(from);
        let take = take as usize;
        if start_idx >= src.cards.len() || take == 0 {
            return;
        }
        let cards: Vec<Card> = src.cards[start_idx..start_idx + take].to_vec();
        let (sx, sy) = src.position_for(start_idx);
        let dst = piles.get(to);
        // Where the head card would land after the move — Pile's
        // position_for evaluates the next slot using the dst's current
        // top card as `prev`, so it returns the right fan position for
        // both empty and non-empty destinations.
        let (dx, dy) = dst.position_for(dst.cards.len());
        self.hint_anim = Some(HintAnim {
            cards,
            src_x: sx,
            src_y: sy,
            dst_x: dx,
            dst_y: dy,
            card_w: src.card_w,
            card_h: src.card_h,
            fan_scale: src.fan_scale,
            start_at: web_time::Instant::now(),
            slide_dur: std::time::Duration::from_millis(550),
            fade_dur: std::time::Duration::from_millis(300),
        });
        agg_gui::animation::request_draw();
    }

    /// Paint the ghost-card preview at its current interpolated
    /// position with the in-flight alpha. No-op when no hint animation
    /// is active.
    pub(super) fn paint_hint_animation(&self, ctx: &mut dyn DrawCtx) {
        let Some(anim) = self.hint_anim.as_ref() else {
            return;
        };
        if anim.done() {
            return;
        }
        let (bx, by, alpha) = anim.current();
        if alpha <= 0.0 {
            return;
        }
        let fan = anim.card_h * crate::piles::FAN_DOWN_FACE_UP * anim.fan_scale;
        ctx.set_global_alpha(alpha);
        for (i, card) in anim.cards.iter().enumerate() {
            let y = by - i as f64 * fan;
            paint_card_at(ctx, card, bx, y, anim.card_w, anim.card_h, &self.atlas);
        }
        ctx.set_global_alpha(1.0);
    }
}
