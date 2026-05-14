//! Card sprite atlas — each visible unique card appearance is rasterised
//! lazily into an `Arc<Vec<u8>>` of straight-alpha RGBA8 pixels at exactly
//! the physical pixel size it'll be blitted to.
//!
//! At runtime, `paint_pile` blits those bytes via
//! `DrawCtx::draw_image_rgba_arc`. The wgpu backend keys its GPU texture
//! cache by `Arc::as_ptr`, so cloning an Arc across many cards shares
//! one GPU texture — every face-down card on the board reuses the same
//! card-back texture, every Ace of Spades shares one texture, etc.
//!
//! The atlas is replaced by `GameWidget` whenever the effective render
//! scale (playfield_scale × device DPR) changes. Cards come from the
//! bundled CC0 SVG deck (see `super::svg_deck`); the master SVG is
//! rasterised only when the first sprite at that size is requested, and
//! each face/back sprite is cropped only if paint actually asks for it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use agg_gui::text::Font;

use crate::cards::{Rank, Suit};

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
    /// Logical card dimensions this atlas was built for. Stored so
    /// `GameWidget` can detect when the active variant's per-pile
    /// `card_w / card_h` no longer matches and a rebuild is required
    /// (e.g. switching from Klondike's 90×126 to Mom's 70×98).
    pub card_w_logical: f64,
    pub card_h_logical: f64,
    /// Effective render scale this atlas was built at
    /// (`playfield_scale × device_scale`). Used by `GameWidget` to
    /// detect when a rebuild is required.
    pub render_scale: f64,
    deck: RefCell<Option<DeckBitmap>>,
    faces: RefCell<HashMap<FaceKey, Arc<Vec<u8>>>>,
    back: RefCell<Option<Arc<Vec<u8>>>>,
}

impl CardSpriteAtlas {
    /// Create an empty sprite cache for exactly
    /// `card_w_logical * scale × card_h_logical * scale` physical
    /// pixels. The bundled CC0 SVG deck and individual card sprites are
    /// built lazily on first paint; the `font` argument is unused today
    /// but kept for API stability while we may layer procedural art on
    /// top later.
    pub fn build(_font: &Arc<Font>, card_w_logical: f64, card_h_logical: f64, scale: f64) -> Self {
        let scale = scale.max(0.5);
        let px_w = (card_w_logical * scale).round().max(1.0) as u32;
        let px_h = (card_h_logical * scale).round().max(1.0) as u32;

        Self {
            px_w,
            px_h,
            card_w_logical,
            card_h_logical,
            render_scale: scale,
            deck: RefCell::new(None),
            faces: RefCell::new(HashMap::new()),
            back: RefCell::new(None),
        }
    }

    pub fn face(&self, suit: Suit, rank: Rank) -> Arc<Vec<u8>> {
        let key = FaceKey { suit, rank };
        if let Some(sprite) = self.faces.borrow().get(&key).cloned() {
            return sprite;
        }

        let sprite = Arc::new(self.with_deck(|deck| deck.extract_face(suit, rank)));
        self.faces.borrow_mut().insert(key, sprite.clone());
        sprite
    }

    pub fn back(&self) -> Arc<Vec<u8>> {
        if let Some(sprite) = self.back.borrow().clone() {
            return sprite;
        }

        let sprite = Arc::new(self.with_deck(DeckBitmap::extract_back));
        *self.back.borrow_mut() = Some(sprite.clone());
        sprite
    }

    fn with_deck<T>(&self, f: impl FnOnce(&DeckBitmap) -> T) -> T {
        if self.deck.borrow().is_none() {
            *self.deck.borrow_mut() = Some(self.build_deck());
        }
        let deck = self.deck.borrow();
        f(deck
            .as_ref()
            .expect("deck populated before extracting card sprite"))
    }

    fn build_deck(&self) -> DeckBitmap {
        // On desktop the wgpu surface lands texture pixels 1:1 on
        // physical RGB-stripe LCD subpixels, so the SVG goes through a
        // 3×-horizontal "LCD-RGB back buffer" + 5-tap subpixel filter
        // for a small horizontal-resolution boost. On WASM (and any
        // other non-RGB-stripe target) the browser / display path
        // resamples the texture, which would smear the subpixel
        // pattern and add chroma fringing, so stick with the plain
        // RGBA raster there.
        #[cfg(not(target_arch = "wasm32"))]
        {
            DeckBitmap::build_lcd(self.px_w, self.px_h)
        }
        #[cfg(target_arch = "wasm32")]
        {
            DeckBitmap::build(self.px_w, self.px_h)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;
    use std::time::Instant;

    use agg_gui::text::Font;

    use super::*;

    const FONT_BYTES: &[u8] = include_bytes!("../../assets/CascadiaCode.ttf");

    fn test_font() -> Arc<Font> {
        Arc::new(Font::from_slice(FONT_BYTES).expect("solitaire default font"))
    }

    fn eager_like_old_build(px_w: u32, px_h: u32) -> usize {
        #[cfg(not(target_arch = "wasm32"))]
        let deck = DeckBitmap::build_lcd(px_w, px_h);
        #[cfg(target_arch = "wasm32")]
        let deck = DeckBitmap::build(px_w, px_h);

        let mut bytes = deck.extract_back().len();
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                bytes += deck.extract_face(suit, rank).len();
            }
        }
        black_box(bytes)
    }

    fn elapsed_ms(t0: Instant) -> f64 {
        t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Diagnostic perf probe for resize behavior. Run with:
    /// `cargo test -p solitaire-core measure_lazy_atlas_resize_work -- --ignored --nocapture`
    #[test]
    #[ignore = "diagnostic timing probe; use --nocapture to inspect resize costs"]
    fn measure_lazy_atlas_resize_work() {
        let font = test_font();
        let card_w = 90.0;
        let card_h = 126.0;
        let dpr = 1.0;

        // Exclude one-time gzip inflation from resize measurements; the
        // running app has already paid this after the first deck render.
        black_box(eager_like_old_build(1, 1));

        let t0 = Instant::now();
        let atlas = CardSpriteAtlas::build(&font, card_w, card_h, dpr);
        println!(
            "lazy reset only: {:.2} ms ({}x{} px)",
            elapsed_ms(t0),
            atlas.px_w,
            atlas.px_h
        );

        let t0 = Instant::now();
        let back = atlas.back();
        println!(
            "lazy first back: {:.2} ms ({} bytes)",
            elapsed_ms(t0),
            black_box(back.len())
        );

        let visible = [
            (Suit::Spades, Rank::Ace),
            (Suit::Hearts, Rank::Two),
            (Suit::Diamonds, Rank::Three),
            (Suit::Clubs, Rank::Four),
            (Suit::Spades, Rank::Five),
            (Suit::Hearts, Rank::Six),
            (Suit::Diamonds, Rank::Seven),
        ];
        let t0 = Instant::now();
        let mut visible_bytes = 0usize;
        for (suit, rank) in visible {
            visible_bytes += atlas.face(suit, rank).len();
        }
        println!(
            "lazy 7 additional faces: {:.2} ms ({} bytes)",
            elapsed_ms(t0),
            black_box(visible_bytes)
        );

        let t0 = Instant::now();
        let mut full_deck_bytes = back.len();
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                full_deck_bytes += atlas.face(suit, rank).len();
            }
        }
        println!(
            "lazy fill remaining deck: {:.2} ms ({} bytes total)",
            elapsed_ms(t0),
            black_box(full_deck_bytes)
        );

        let px_w = (card_w * dpr).round().max(1.0) as u32;
        let px_h = (card_h * dpr).round().max(1.0) as u32;
        let t0 = Instant::now();
        let eager_bytes = eager_like_old_build(px_w, px_h);
        println!(
            "old eager-equivalent build: {:.2} ms ({} bytes)",
            elapsed_ms(t0),
            eager_bytes
        );
    }
}
