//! CC0 SVG playing-card deck — bundled master file from Wikimedia Commons
//! ("English pattern playing cards deck PLUS CC0", by Loren Osborn / Dmitry
//! Fomin / Guy vandegrift, dedicated to the public domain). The single
//! source SVG arranges all 54 cards in a 13 × 5 grid; we rasterize the
//! whole thing once at the desired card pixel size and crop sub-rects
//! into per-card sprites for the atlas.
//!
//! Grid layout (per the Commons file's own `<dc:description>`):
//! - Row 0 = Spades, Row 1 = Hearts, Row 2 = Diamonds, Row 3 = Clubs.
//!   In each suited row the columns run Ace..King left to right.
//! - Row 4 (ancillary): col 0 = blue rounded-rectangle card back, cols
//!   1–2 = jokers, col 3 = blank, cols 4–7 = symmetric backs in
//!   blue / red / purple / gold.
//!
//! License: CC0 1.0 Universal Public Domain Dedication. No attribution
//! required, but we credit upstream in this file's header for goodwill.

use agg_gui::framebuffer::unpremultiply_rgba_inplace;
use agg_gui::svg::{parse_svg, render_svg_tree_to_framebuffer_at_size, SvgParseOptions};

use crate::cards::{Rank, Suit};

/// Bundled master SVG (~7.6 MB). Parsed once per atlas rebuild.
const MASTER_SVG: &[u8] = include_bytes!("../../assets/cards/english_pattern_cc0.svg");

/// Card grid dimensions in the master SVG.
const COLS: u32 = 13;
const ROWS: u32 = 5;

/// Rasterized grid of `(COLS × ROWS)` cards at exactly `(card_px_w,
/// card_px_h)` per cell. Rows are top-down, in straight-alpha RGBA8.
pub struct DeckBitmap {
    pub pixels: Vec<u8>,
    pub master_w: u32,
    pub card_px_w: u32,
    pub card_px_h: u32,
}

impl DeckBitmap {
    /// Parse the bundled SVG and rasterize the whole deck at exactly
    /// `(card_px_w * COLS, card_px_h * ROWS)` pixels. The aspect ratio
    /// of the source cells (≈0.682) differs slightly from our card
    /// dimensions (≈0.714), so the rendered art is squished horizontally
    /// by ≈5% — visually unnoticeable for most viewers.
    pub fn build(card_px_w: u32, card_px_h: u32) -> Self {
        let master_w = card_px_w * COLS;
        let master_h = card_px_h * ROWS;
        let tree = parse_svg(MASTER_SVG, &SvgParseOptions::default())
            .expect("bundled CC0 deck SVG parses");
        let fb = render_svg_tree_to_framebuffer_at_size(&tree, master_w, master_h)
            .expect("bundled CC0 deck SVG rasterizes");
        let mut pixels = fb.pixels_flipped();
        unpremultiply_rgba_inplace(&mut pixels);
        debug_assert_eq!(pixels.len(), (master_w * master_h * 4) as usize);
        Self {
            pixels,
            master_w,
            card_px_w,
            card_px_h,
        }
    }

    /// Copy the cell at `(row, col)` into a fresh, owned RGBA8 sprite.
    pub fn extract(&self, row: u32, col: u32) -> Vec<u8> {
        debug_assert!(row < ROWS && col < COLS, "({row}, {col}) outside grid");
        let bpp = 4u32;
        let src_stride = self.master_w * bpp;
        let dst_stride = self.card_px_w * bpp;
        let src_x = col * self.card_px_w;
        let src_y = row * self.card_px_h;
        let mut out = vec![0u8; (self.card_px_w * self.card_px_h * bpp) as usize];
        for y in 0..self.card_px_h {
            let src_off = ((src_y + y) * src_stride + src_x * bpp) as usize;
            let dst_off = (y * dst_stride) as usize;
            out[dst_off..dst_off + dst_stride as usize]
                .copy_from_slice(&self.pixels[src_off..src_off + dst_stride as usize]);
        }
        out
    }

    pub fn extract_face(&self, suit: Suit, rank: Rank) -> Vec<u8> {
        let (row, col) = position_for(suit, rank);
        self.extract(row, col)
    }

    /// The default card back is the blue rounded-rectangle in the
    /// ancillary row at column 0 (under the Aces).
    pub fn extract_back(&self) -> Vec<u8> {
        self.extract(4, 0)
    }
}

fn position_for(suit: Suit, rank: Rank) -> (u32, u32) {
    let row = match suit {
        Suit::Spades => 0,
        Suit::Hearts => 1,
        Suit::Diamonds => 2,
        Suit::Clubs => 3,
    };
    let col = (rank as u32) - 1; // Rank::Ace=1 → col 0, Rank::King=13 → col 12.
    (row, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_corners() {
        assert_eq!(position_for(Suit::Spades, Rank::Ace), (0, 0));
        assert_eq!(position_for(Suit::Spades, Rank::King), (0, 12));
        assert_eq!(position_for(Suit::Clubs, Rank::Ace), (3, 0));
        assert_eq!(position_for(Suit::Clubs, Rank::King), (3, 12));
        assert_eq!(position_for(Suit::Diamonds, Rank::Seven), (2, 6));
    }

    /// Sanity check that the bundled SVG parses and rasterizes at a
    /// small size, and that face/back extraction returns the expected
    /// byte counts. Slow (parses 7.6 MB of XML) but a one-time guard
    /// against the asset getting corrupted or usvg dropping support.
    #[test]
    #[ignore = "parses the 7.6 MB CC0 deck — opt in via `--ignored`"]
    fn deck_rasterizes_and_extracts() {
        let bmp = DeckBitmap::build(20, 28);
        assert_eq!(bmp.master_w, 20 * COLS);
        assert_eq!(bmp.pixels.len(), (20 * COLS * 28 * ROWS * 4) as usize);
        let face = bmp.extract_face(Suit::Hearts, Rank::Queen);
        assert_eq!(face.len(), (20 * 28 * 4) as usize);
        let back = bmp.extract_back();
        assert_eq!(back.len(), (20 * 28 * 4) as usize);
    }
}
