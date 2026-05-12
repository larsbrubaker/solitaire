//! Card-flight animation primitives.
//!
//! Two flavours of animation live here:
//!
//! 1. `CardAnim` — a single card flying from `src` to `dst` with a 3-D
//!    Y-axis flip. Used for Spider's stock dispense (10 cards to 10
//!    cascades) and Klondike's stock-to-waste click. Renders as a
//!    perspective-projected textured quad.
//! 2. `DeckFlipAnim` — the WHOLE waste pile flipping back onto the
//!    stock at the end of a Klondike deal cycle. Renders as a real
//!    3-D BOX (top face = current top-of-waste, bottom face = card
//!    back, side face = alternating stripes representing the stacked
//!    card edges). Thickness scales with card count, so a 30-card
//!    waste flips as a chunky deck while a 3-card waste flips as a
//!    thin pamphlet.
//!
//! Game state mutates immediately; the animations are purely cosmetic
//! and `GameWidget` paints them on top each frame, hiding the freshly-
//! placed destination cards from the static pile paint while their
//! in-flight twins animate over.

use std::sync::Arc;
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

/// Klondike stock-recycle animation: the whole waste pile flips back
/// onto the stock as one 3-D box. Thickness scales with card count.
/// Painted as up to 4 textured quads per frame (top face, bottom
/// face, two side faces — the visible ones only).
#[derive(Clone, Debug)]
pub struct DeckFlipAnim {
    /// Top card of the waste before the flip — its FRONT face shows
    /// on the top of the deck at θ = 0. After the flip the back
    /// faces the camera (deck is now the stock).
    pub top_card: Card,
    /// Number of cards in the flipping deck — drives both the deck's
    /// physical thickness AND the number of stripes on the side
    /// texture.
    pub card_count: u32,
    /// One-card dimensions (screen px).
    pub card_w: f64,
    pub card_h: f64,
    /// Total deck thickness (screen px). Computed from `card_count`
    /// at construction time so a 30-card waste flips as a chunky
    /// stack and a 3-card waste flips as a thin pamphlet.
    pub thickness: f64,
    /// Screen-space CENTER positions (deck centroid). Source =
    /// waste, destination = stock; the deck translates and flips in
    /// the same gesture.
    pub src_center_x: f64,
    pub src_center_y: f64,
    pub dst_center_x: f64,
    pub dst_center_y: f64,
    pub start_at: Instant,
    pub duration: Duration,
    /// Destination pile id (the stock) — hide its top cards from
    /// the static pile paint while the in-flight deck is visible.
    pub dst_pile: PileId,
    pub dst_hide_from: usize,
    /// Pre-rasterised side-stripe texture: alternating dark/light
    /// columns, one band per card. The texture is held by `Arc` so
    /// the wgpu Arc-keyed texture cache de-duplicates across frames.
    pub side_texture: Arc<Vec<u8>>,
    pub side_tex_w: u32,
    pub side_tex_h: u32,
}

impl DeckFlipAnim {
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

    pub fn has_started(&self) -> bool {
        Instant::now() >= self.start_at
    }
}

/// Compute the deck thickness in screen-px given a card count and the
/// rendered card width. Real cards are ~0.3 mm thick, a 52-card deck
/// is about 15 mm = ~17 % of a card's width. We exaggerate slightly
/// so the side stripes read at small card sizes: 35 % of `card_w` for
/// 52 cards, linear in count, with a tiny minimum so a 1-card "flip"
/// still has a visible volume.
pub fn deck_thickness_for(card_count: u32, card_w: f64) -> f64 {
    let max = card_w * 0.35;
    let frac = (card_count as f64 / 52.0).min(1.0);
    (max * frac).max(card_w * 0.02)
}

/// Build the procedural side-stripe texture for a `card_count` deck.
/// One vertical band per card, alternating between a dark band
/// (card-edge in shadow) and a light band (card-edge in light). The
/// resulting image is `2*card_count` × 4 px straight-alpha RGBA8,
/// which stretches cleanly to whatever face dimensions the renderer
/// hands it.
pub fn build_stripe_texture(card_count: u32) -> (Arc<Vec<u8>>, u32, u32) {
    let cards = card_count.max(1);
    // Two pixels per card-stripe so even the thinnest deck renders a
    // visible alternation at moderate scales.
    let img_w = (cards * 2).max(4);
    let img_h: u32 = 4;
    let mut pixels = Vec::with_capacity((img_w * img_h * 4) as usize);
    // Card-stock-cream + edge-shadow grey. Both fully opaque.
    const LIGHT: [u8; 4] = [0xF2, 0xEC, 0xDB, 0xFF];
    const DARK: [u8; 4] = [0x7A, 0x70, 0x60, 0xFF];
    for _ in 0..img_h {
        for x in 0..img_w {
            // 2 px per card → bit 1 of (x / 2) picks the band.
            let stripe = (x / 2) % 2 == 0;
            let c = if stripe { LIGHT } else { DARK };
            pixels.extend_from_slice(&c);
        }
    }
    (Arc::new(pixels), img_w, img_h)
}

/// Which face of the deck-flip box a `DeckFaceDraw` represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckFace {
    Top,
    Bottom,
    LeftSide,
    RightSide,
}

/// One face of the flipping deck box, ready to draw. `corners` are in
/// screen coordinates (Y-up, BL/BR/TR/TL order) so `draw_image_rgba_
/// corners` can submit the quad directly. `face` tells the caller
/// which texture to bind — top = waste's top card front face, bottom
/// = card back, sides = the procedural stripe texture.
#[derive(Clone, Copy, Debug)]
pub struct DeckFaceDraw {
    pub face: DeckFace,
    pub corners: [(f64, f64); 4],
    /// Painter's-algorithm sort key — average Z of the four corners
    /// AFTER rotation. Smaller (more negative / further from camera)
    /// paints first.
    pub depth: f64,
}

/// Produce the list of currently-visible faces for `anim` in the
/// correct paint order (back-to-front). Rotation is 0 → π around the
/// Y axis; perspective focal length is 4× card height (matches the
/// single-card flip).
pub fn animated_deck_faces(anim: &DeckFlipAnim) -> Vec<DeckFaceDraw> {
    let t = anim.progress();
    let eased = ease_out_cubic(t);
    let cx = anim.src_center_x + (anim.dst_center_x - anim.src_center_x) * eased;
    // Add a small lift so the deck arcs up and back down.
    let cy_base = anim.src_center_y + (anim.dst_center_y - anim.src_center_y) * eased;
    let dx = anim.dst_center_x - anim.src_center_x;
    let dy = anim.dst_center_y - anim.src_center_y;
    let dist = (dx * dx + dy * dy).sqrt().max(anim.card_h * 0.5);
    let arc = (t * std::f64::consts::PI).sin() * dist * 0.12;
    let cy = cy_base + arc;

    let half_w = anim.card_w / 2.0;
    let half_h = anim.card_h / 2.0;
    let half_d = anim.thickness / 2.0;
    let angle = t * std::f64::consts::PI;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let focal = anim.card_h * 4.0;

    // Rotate one local corner (x, y, z) around the Y axis and project
    // with perspective into the deck's screen-space frame at (cx, cy).
    let project = |lx: f64, ly: f64, lz: f64| -> ((f64, f64), f64) {
        let x3 = lx * cos_a + lz * sin_a;
        let z3 = -lx * sin_a + lz * cos_a;
        let denom = 1.0 - z3 / focal;
        let safe = if denom.abs() < 1e-4 { 1e-4 } else { denom };
        ((cx + x3 / safe, cy + ly / safe), z3)
    };

    // 8 box corners labelled by sign on each axis. Suffix order is
    // X, Y, Z.
    let nnn = project(-half_w, -half_h, -half_d);
    let pnn = project(half_w, -half_h, -half_d);
    let ppn = project(half_w, half_h, -half_d);
    let npn = project(-half_w, half_h, -half_d);
    let nnp = project(-half_w, -half_h, half_d);
    let pnp = project(half_w, -half_h, half_d);
    let ppp = project(half_w, half_h, half_d);
    let npp = project(-half_w, half_h, half_d);

    // Average rotated-z for each face — used as the painter sort key.
    let avg_z = |a: f64, b: f64, c: f64, d: f64| -> f64 { (a + b + c + d) * 0.25 };

    let mut faces: Vec<DeckFaceDraw> = Vec::with_capacity(4);

    // Top face (+Z normal): visible while cos θ > 0. Corners
    // BL=nnp, BR=pnp, TR=ppp, TL=npp.
    if cos_a > 0.0 {
        faces.push(DeckFaceDraw {
            face: DeckFace::Top,
            corners: [nnp.0, pnp.0, ppp.0, npp.0],
            depth: avg_z(nnp.1, pnp.1, ppp.1, npp.1),
        });
    }
    // Bottom face (-Z normal): visible while cos θ < 0. Swap BL/BR
    // and TL/TR so the back texture isn't mirrored. Corners viewed
    // from outside the box: BL=pnn, BR=nnn, TR=npn, TL=ppn.
    if cos_a < 0.0 {
        faces.push(DeckFaceDraw {
            face: DeckFace::Bottom,
            corners: [pnn.0, nnn.0, npn.0, ppn.0],
            depth: avg_z(pnn.1, nnn.1, npn.1, ppn.1),
        });
    }
    // Left side face (-X normal): visible while sin θ > 0 (all of
    // 0 → π). Corners viewed from outside: BL=nnn, BR=nnp, TR=npp,
    // TL=npn.
    if sin_a > 0.0 {
        faces.push(DeckFaceDraw {
            face: DeckFace::LeftSide,
            corners: [nnn.0, nnp.0, npp.0, npn.0],
            depth: avg_z(nnn.1, nnp.1, npp.1, npn.1),
        });
    }
    // Right side face (+X normal): visible while sin θ < 0 (never on
    // a 0 → π flip, but reserved for the reverse direction). Corners
    // viewed from outside: BL=pnp, BR=pnn, TR=ppn, TL=ppp.
    if sin_a < 0.0 {
        faces.push(DeckFaceDraw {
            face: DeckFace::RightSide,
            corners: [pnp.0, pnn.0, ppn.0, ppp.0],
            depth: avg_z(pnp.1, pnn.1, ppn.1, ppp.1),
        });
    }

    // Painter's algorithm: paint back-to-front. Smallest (most
    // negative) `depth` first.
    faces.sort_by(|a, b| {
        a.depth
            .partial_cmp(&b.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    faces
}
