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

/// Real-3D projected card snapshot — four screen-space corners of
/// the card after applying Y-axis rotation and perspective division.
/// `GameWidget` draws the card as a textured quad with these corners,
/// so the rendering is genuine 3D (perspective foreshortening, the
/// card looks thinner near the camera-far edge) rather than a 2D
/// horizontal-squash trick.
#[derive(Clone, Copy, Debug)]
pub struct AnimQuad {
    /// Four corners in SCREEN coordinates, ordered bottom-left,
    /// bottom-right, top-right, top-left (Y-up convention to match
    /// the rest of the widget tree).
    pub corners: [(f64, f64); 4],
    /// `true` if this frame should sample the FRONT face; `false`
    /// for the back. Swaps at the halfway point of the rotation so
    /// the texture is never mirrored.
    pub show_front: bool,
}

/// Project the card into screen space for the current animation
/// frame. Position interpolates with cubic ease-out + a parabolic
/// arc; the card itself rotates 0→180° around its vertical axis with
/// a small perspective focal length so the foreshortening is
/// visible at oblique angles.
pub fn animated_quad(anim: &CardAnim) -> AnimQuad {
    let t = anim.progress();
    let eased = ease_out_cubic(t);
    let x_base = anim.src_x + (anim.dst_x - anim.src_x) * eased;
    let y_base = anim.src_y + (anim.dst_y - anim.src_y) * eased;
    // Parabolic arc — lifts the card mid-flight, settles to 0 at
    // both endpoints. Magnitude scales with travel distance so a
    // short hop barely lifts, a 10-cascade Spider deal arcs high.
    let dx = anim.dst_x - anim.src_x;
    let dy = anim.dst_y - anim.src_y;
    let dist = (dx * dx + dy * dy).sqrt();
    let arc = (t * std::f64::consts::PI).sin() * dist * 0.12;

    let cx = x_base + anim.card_w / 2.0;
    let cy = y_base + anim.card_h / 2.0 + arc;
    let half_w = anim.card_w / 2.0;
    let half_h = anim.card_h / 2.0;

    let angle = if anim.flip {
        t * std::f64::consts::PI
    } else {
        0.0
    };
    let show_front = if anim.flip {
        t >= 0.5
    } else {
        anim.card.face_up
    };

    // 3-D projection: each corner sits at (±half_w, ±half_h, 0) in
    // card-local space. Rotate around the Y axis by `angle`, then
    // divide x/y by (1 - z / focal) for a perspective foreshortening.
    // Focal length is the rendered card-height — strong enough that
    // a near-edge view shows real trapezoidal distortion, weak enough
    // that the face-on extremes still read as a near-rectangle.
    let focal = anim.card_h * 4.0;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let project = |lx: f64, ly: f64| -> (f64, f64) {
        // Y-axis rotation: x' = x*cos + z*sin, z' = -x*sin + z*cos.
        // With z_local = 0: x' = x*cos, z' = -x*sin.
        let x3 = lx * cos_a;
        let z3 = -lx * sin_a;
        let denom = 1.0 - z3 / focal;
        let safe = if denom.abs() < 1e-4 { 1e-4 } else { denom };
        (cx + x3 / safe, cy + ly / safe)
    };
    // Corner order: BL, BR, TR, TL (Y-up).
    let corners = [
        project(-half_w, -half_h),
        project(half_w, -half_h),
        project(half_w, half_h),
        project(-half_w, half_h),
    ];
    AnimQuad {
        corners,
        show_front,
    }
}
