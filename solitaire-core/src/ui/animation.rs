//! Card-flight animation primitives — currently driving the Spider /
//! Klondike stock-click dispense. The move is applied to game state
//! immediately (so undo and rules stay consistent), and a `CardAnim`
//! captures the cosmetic transit: a card flies from `src` to `dst`
//! while rotating around its vertical axis (Y-axis 3-D flip), so the
//! face changes from back → front mid-flight.
//!
//! `GameWidget` owns a `Vec<CardAnim>`, hides the freshly-placed
//! destination card from the static pile paint while the animation is
//! in flight, and paints the in-flight card on top each frame.

use std::time::Duration;
use web_time::Instant;

use crate::cards::Card;
use crate::piles::PileId;

/// One card-in-flight animation entry.
#[derive(Clone, Debug)]
pub struct CardAnim {
    pub card: Card,
    /// Source rect in screen coords (where the card started).
    pub src_x: f64,
    pub src_y: f64,
    /// Destination rect in screen coords (where the card lands).
    pub dst_x: f64,
    pub dst_y: f64,
    /// Card size — must match the destination pile's `card_w` /
    /// `card_h` so the landed card visually swaps in seamlessly.
    pub card_w: f64,
    pub card_h: f64,
    /// When this animation begins. Staggered start times let a 10-
    /// card Spider dispense look like 10 cards dealt in sequence.
    pub start_at: Instant,
    pub duration: Duration,
    /// Pile that owns the destination card once landed. While the
    /// animation is in flight, `GameWidget::paint` hides the top card
    /// of this pile from the static pile paint so we don't double-
    /// draw it.
    pub dst_pile: PileId,
    /// Index in `dst_pile` that should be hidden during this
    /// animation. `Some(N)` means hide cards at indices `>= N`. Set
    /// to `pile.len() - 1` for "hide the freshly-added top card".
    pub dst_hide_from: usize,
    /// Whether to render the Y-axis 3-D flip (back-to-front halfway
    /// through). `false` keeps the face static — used when a card is
    /// already face-up at both endpoints and only the position moves.
    pub flip: bool,
}

impl CardAnim {
    /// Progress in `[0, 1]`, clamped — `0` before `start_at`, `1`
    /// after `start_at + duration`. The runner removes entries that
    /// reach `1.0` on the next paint.
    pub fn progress(&self) -> f64 {
        let now = Instant::now();
        if now < self.start_at {
            return 0.0;
        }
        let elapsed = now.duration_since(self.start_at).as_secs_f64();
        let dur = self.duration.as_secs_f64().max(1e-3);
        (elapsed / dur).clamp(0.0, 1.0)
    }

    pub fn done(&self) -> bool {
        self.progress() >= 1.0
    }

    /// Whether this animation has started (true `progress > 0` is
    /// already true; this is the strict "we should start drawing /
    /// stop showing the destination card" flag — i.e. the
    /// `start_at` instant has passed).
    pub fn has_started(&self) -> bool {
        Instant::now() >= self.start_at
    }
}

/// Cubic ease-out — fast start, gentle finish. Feels natural for a
/// card "landing" on its destination.
pub fn ease_out_cubic(t: f64) -> f64 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// Render-time transform for an in-flight card. Returns the screen-
/// space top-left position, the horizontal scale factor (cosine of
/// the flip angle), and whether to draw the FRONT face (true) or the
/// BACK (false).
pub fn animated_transform(anim: &CardAnim) -> AnimTransform {
    let t = anim.progress();
    let eased = ease_out_cubic(t);
    let x = anim.src_x + (anim.dst_x - anim.src_x) * eased;
    let y = anim.src_y + (anim.dst_y - anim.src_y) * eased;
    if !anim.flip {
        return AnimTransform {
            x,
            y,
            scale_x: 1.0,
            show_front: anim.card.face_up,
        };
    }
    // The card rotates 180° about its vertical axis over the
    // animation. cos(0) = 1 → full width; cos(π/2) = 0 → edge-on;
    // cos(π) = -1 → full width again but mirrored. We take the
    // absolute value for `scale_x` and swap the face at the halfway
    // point so the texture never paints mirrored.
    let angle = t * std::f64::consts::PI;
    let scale_x = angle.cos().abs();
    let show_front = t >= 0.5;
    AnimTransform {
        x,
        y,
        scale_x,
        show_front,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnimTransform {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub show_front: bool,
}
