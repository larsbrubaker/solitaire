//! Card sprite atlas — each unique card appearance is rasterised ONCE
//! into an `Arc<Vec<u8>>` of straight-alpha RGBA8 pixels.
//!
//! At runtime, `paint_pile` blits those bytes via
//! `DrawCtx::draw_image_rgba_arc`. The wgpu backend keys its GPU texture
//! cache by `Arc::as_ptr`, so cloning an Arc across many cards shares
//! one GPU texture — every face-down card on the board reuses the same
//! card-back texture, every Ace of Spades shares one texture, etc.
//!
//! This collapses ~200+ AGG primitives per frame into ~50 textured-quad
//! draws and lets us push 10,000+ cards per frame on the hardware path.

use std::collections::HashMap;
use std::sync::Arc;

use agg_gui::framebuffer::unpremultiply_rgba_inplace;
use agg_gui::text::Font;
use agg_gui::{Framebuffer, GfxCtx};

use crate::cards::{Card, Rank, Suit};
use crate::consts::{CARD_H, CARD_W};

use super::card_back::paint_card_back;
use super::card_face::paint_card_face;

/// Render the offscreen sprites at this many physical pixels per logical
/// pixel. 2× gives crisp text on standard-DPI displays without paying
/// for the full font shaping every frame; the 4× memory cost (52 ×
/// 90×126×4 → 52 × 180×252×4 ≈ 9.4 MB) is fine for native and acceptable
/// for the wasm build.
const ATLAS_OVERSAMPLE: f64 = 2.0;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct FaceKey {
    suit: Suit,
    rank: Rank,
}

pub struct CardSpriteAtlas {
    /// Sprite pixel size (atlas-internal). Always
    /// `(CARD_W * ATLAS_OVERSAMPLE, CARD_H * ATLAS_OVERSAMPLE)`.
    pub px_w: u32,
    pub px_h: u32,
    faces: HashMap<FaceKey, Arc<Vec<u8>>>,
    back: Arc<Vec<u8>>,
}

impl CardSpriteAtlas {
    /// Pre-rasterise all 52 card faces + 1 card back. Call once at
    /// startup; clone the returned Arc and pass it to widgets.
    pub fn build(font: &Arc<Font>) -> Arc<Self> {
        let px_w = (CARD_W * ATLAS_OVERSAMPLE).round() as u32;
        let px_h = (CARD_H * ATLAS_OVERSAMPLE).round() as u32;

        let back = rasterise(px_w, px_h, |ctx| {
            paint_card_back(ctx, 0.0, 0.0, CARD_W, CARD_H);
        });

        let mut faces = HashMap::with_capacity(52);
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                let card = Card {
                    suit,
                    rank,
                    face_up: true,
                    deck_id: 0,
                };
                let pixels = rasterise(px_w, px_h, |ctx| {
                    paint_card_face(ctx, &card, 0.0, 0.0, CARD_W, CARD_H, font);
                });
                faces.insert(FaceKey { suit, rank }, pixels);
            }
        }

        Arc::new(Self {
            px_w,
            px_h,
            faces,
            back,
        })
    }

    pub fn face(&self, suit: Suit, rank: Rank) -> Arc<Vec<u8>> {
        // `expect` is safe because `build` populates every (suit, rank).
        self.faces
            .get(&FaceKey { suit, rank })
            .expect("every face cached at build()")
            .clone()
    }

    pub fn back(&self) -> Arc<Vec<u8>> {
        self.back.clone()
    }
}

/// Rasterise `paint_fn` into a fresh `Framebuffer` of `(px_w, px_h)`
/// pixels. The atlas-internal coordinate space is the LOGICAL card size
/// `(CARD_W, CARD_H)` — `paint_fn` draws as if the canvas were that big,
/// and we pre-scale the framebuffer by `ATLAS_OVERSAMPLE` so the
/// rasterised pixels capture sub-logical-pixel detail.
fn rasterise(
    px_w: u32,
    px_h: u32,
    paint_fn: impl FnOnce(&mut GfxCtx<'_>),
) -> Arc<Vec<u8>> {
    let mut fb = Framebuffer::new(px_w, px_h);
    {
        let mut ctx = GfxCtx::new(&mut fb);
        ctx.scale(ATLAS_OVERSAMPLE, ATLAS_OVERSAMPLE);
        paint_fn(&mut ctx);
    }
    let mut pixels = fb.into_pixels();
    // AGG writes premultiplied alpha; draw_image_rgba_arc expects
    // straight alpha. Convert at the cache boundary so blits are cheap.
    unpremultiply_rgba_inplace(&mut pixels);
    Arc::new(pixels)
}
