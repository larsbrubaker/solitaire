//! Card sprite atlas — each unique card appearance is rasterised ONCE
//! into an `Arc<Vec<u8>>` of straight-alpha RGBA8 pixels at exactly the
//! physical pixel size it'll be blitted to.
//!
//! At runtime, `paint_pile` blits those bytes via
//! `DrawCtx::draw_image_rgba_arc`. The wgpu backend keys its GPU texture
//! cache by `Arc::as_ptr`, so cloning an Arc across many cards shares
//! one GPU texture — every face-down card on the board reuses the same
//! card-back texture, every Ace of Spades shares one texture, etc.
//!
//! The atlas is rebuilt by `GameWidget` whenever the effective render
//! scale (playfield_scale × device DPR) changes. Cards come from the
//! bundled CC0 SVG deck (see `super::svg_deck`); the master SVG is
//! rasterised once per rebuild and the 52 faces + back are cropped
//! out as individual sprites.

use std::collections::HashMap;
use std::sync::Arc;

use agg_gui::text::Font;

use crate::cards::{Rank, Suit};
use crate::consts::{CARD_H, CARD_W};

use super::svg_deck::DeckBitmap;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct FaceKey {
    suit: Suit,
    rank: Rank,
}

pub struct CardSpriteAtlas {
    /// Sprite pixel size (atlas-internal) — equals the physical pixel
    /// area the card will occupy on screen.
    pub px_w: u32,
    pub px_h: u32,
    /// Effective render scale this atlas was built at
    /// (`playfield_scale × device_scale`). Used by `GameWidget` to
    /// detect when a rebuild is required.
    pub render_scale: f64,
    faces: HashMap<FaceKey, Arc<Vec<u8>>>,
    back: Arc<Vec<u8>>,
}

impl CardSpriteAtlas {
    /// Pre-rasterise all 52 card faces + 1 card back at exactly
    /// `CARD_W * scale × CARD_H * scale` physical pixels. The bundled
    /// CC0 SVG deck is parsed and rasterised once into a master
    /// bitmap, then sliced into per-card sprites; the `font` argument
    /// is unused today but kept for API stability while we may layer
    /// procedural art (e.g. corner highlights, hints) on top later.
    pub fn build(_font: &Arc<Font>, scale: f64) -> Self {
        let scale = scale.max(0.5);
        let px_w = (CARD_W * scale).round().max(1.0) as u32;
        let px_h = (CARD_H * scale).round().max(1.0) as u32;

        let deck = DeckBitmap::build(px_w, px_h);
        let back = Arc::new(deck.extract_back());

        let mut faces = HashMap::with_capacity(52);
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                faces.insert(
                    FaceKey { suit, rank },
                    Arc::new(deck.extract_face(suit, rank)),
                );
            }
        }

        Self {
            px_w,
            px_h,
            render_scale: scale,
            faces,
            back,
        }
    }

    pub fn face(&self, suit: Suit, rank: Rank) -> Arc<Vec<u8>> {
        self.faces
            .get(&FaceKey { suit, rank })
            .expect("every face cached at build()")
            .clone()
    }

    pub fn back(&self) -> Arc<Vec<u8>> {
        self.back.clone()
    }
}
