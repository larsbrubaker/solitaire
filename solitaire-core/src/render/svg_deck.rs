//! CC0 SVG playing-card deck — bundled master file from Wikimedia Commons
//! ("English pattern playing cards deck PLUS CC0", by Loren Osborn / Dmitry
//! Fomin / Guy vandegrift, dedicated to the public domain). The single
//! source SVG arranges all 54 cards in a 13 × 5 grid; we render the
//! requested card's source rectangle directly into a card-sized sprite.
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

use std::io::Read;
use std::sync::OnceLock;

use agg_gui::framebuffer::unpremultiply_rgba_inplace;
use agg_gui::svg::{
    parse_svg, render_svg_tree_region_to_framebuffer_at_size, SvgParseOptions, SvgTree,
};
use flate2::read::GzDecoder;

use crate::cards::{Rank, Suit};

/// Bundled master SVG, gzipped to ≈1.9 MB at compile time (the raw SVG is
/// ≈7.6 MB). Inflated lazily by `master_svg()` on first use and cached
/// for the rest of the process, so atlas rebuilds (which fire on every
/// render-scale change) re-use the same uncompressed bytes instead of
/// re-inflating ~7 MB per call.
const MASTER_SVG_GZ: &[u8] = include_bytes!("../../assets/cards/english_pattern_cc0.svg.gz");

/// Lazily-inflated master SVG. The first call decompresses the bundled
/// gzip into an owned `Vec<u8>` cached in a `OnceLock`; subsequent calls
/// return a reference into the same cache.
fn master_svg() -> &'static [u8] {
    static CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            // Source is ≈7.6 MB so pre-size aggressively; growth-only
            // allocators would otherwise pay several `realloc`s for a
            // file of this magnitude.
            let mut out = Vec::with_capacity(8 * 1024 * 1024);
            GzDecoder::new(MASTER_SVG_GZ)
                .read_to_end(&mut out)
                .expect("bundled CC0 deck gzip decompresses");
            out
        })
        .as_slice()
}

/// Card grid dimensions in the master SVG.
const COLS: u32 = 13;
const ROWS: u32 = 5;

// Card layout in the SVG's native viewport. Verified empirically by
// `deck_layout_probe` (see #[ignore]'d test below). The master canvas
// is ≈5109×2883 source units; cards are 359×539, laid out on a
// 390×570 pitch with a 39×30 top-left margin. Cards do NOT fill their
// (master/13 × master/5) cells — there's a transparent gutter between
// adjacent cards (~31px source units), and cumulative pitch differs
// from cell pitch enough that simple `master/13` cropping smears
// adjacent-card content into the wrong sprite. Hence these constants.
const SRC_CARD_W: f64 = 359.0;
const SRC_CARD_H: f64 = 539.0;
const SRC_LEFT_MARGIN: f64 = 39.0;
const SRC_TOP_MARGIN: f64 = 30.0;
const SRC_PITCH_X: f64 = 390.0;
const SRC_PITCH_Y: f64 = 570.0;

/// The Wikimedia master ships with a fully-opaque green felt rectangle
/// covering the whole canvas (line 15917 of the SVG: a `<rect>` with
/// `style="fill:#55aa55;fill-opacity:1"`). Left in place, that paints
/// over the transparent background and shows up as a green halo around
/// every card we crop. Replacing the rect's fill-opacity with 0 — same
/// length so file offsets are preserved — neutralizes the background
/// without touching any card art.
fn strip_background(svg: &[u8]) -> Vec<u8> {
    const NEEDLE: &[u8] = b"fill:#55aa55;fill-opacity:1";
    const REPLACE: &[u8] = b"fill:#55aa55;fill-opacity:0";
    debug_assert_eq!(NEEDLE.len(), REPLACE.len());
    let mut out = svg.to_vec();
    if let Some(pos) = out.windows(NEEDLE.len()).position(|w| w == NEEDLE) {
        out[pos..pos + NEEDLE.len()].copy_from_slice(REPLACE);
    }
    out
}

/// Lazily-parsed, background-stripped SVG tree. Parsing the 7.6 MB source
/// is expensive enough that card-at-a-time rendering must share the tree.
fn deck_tree() -> &'static SvgTree {
    static TREE: OnceLock<SvgTree> = OnceLock::new();
    TREE.get_or_init(|| {
        let svg = strip_background(master_svg());
        parse_svg(&svg, &SvgParseOptions::default()).expect("bundled CC0 deck SVG parses")
    })
}

/// Card-sized renderer for the bundled SVG deck.
pub struct DeckBitmap {
    pub card_px_w: u32,
    pub card_px_h: u32,
    lcd_subpixel: bool,
}

impl DeckBitmap {
    /// Prepare a renderer where each card's source rect (359×539 units)
    /// maps directly to the requested `(card_px_w × card_px_h)` output
    /// pixels — no full-deck raster, no downsampling, and no unused backs
    /// or jokers.
    // Native builds use `build_lcd` (RGB-stripe subpixel) and only
    // reach `build` from `#[cfg(test)]` code; suppress the resulting
    // dead-code warning on non-WASM targets.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn build(card_px_w: u32, card_px_h: u32) -> Self {
        Self {
            card_px_w,
            card_px_h,
            lcd_subpixel: false,
        }
    }

    /// LCD-subpixel build for RGB-stripe desktop displays. Rasterizes
    /// each requested card at 3× horizontal resolution into an "LCD-RGB back
    /// buffer", then collapses to a normal-width RGBA8 atlas with
    /// per-channel offset sampling (R from subcolumn 3x, G from 3x+1,
    /// B from 3x+2) through a `[1, 4, 7, 4, 1] / 17` low-pass filter.
    /// The output shape matches `build()` exactly so the rest of the
    /// atlas / blit path is unchanged — only the pixel content
    /// differs, with a small but visible horizontal-resolution boost
    /// on static cards (edges read at ≈3× the horizontal density of
    /// the pixel grid). Card animations smear the subpixel pattern
    /// across the screen pixel grid, so the benefit is mostly visible
    /// when cards are at rest.
    ///
    /// Only safe to call when texture pixels land 1:1 on physical
    /// RGB-stripe LCD pixels (i.e. desktop wgpu surface at integer
    /// scale). Mobile / browser canvases re-sample the texture
    /// through their own pipelines — there `build()` (no subpixel
    /// trickery) is the right choice.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn build_lcd(card_px_w: u32, card_px_h: u32) -> Self {
        Self {
            card_px_w,
            card_px_h,
            lcd_subpixel: true,
        }
    }

    /// Source position of card `(row, col)`'s top-left corner inside the
    /// SVG master, accounting for the SVG's per-card pitch and
    /// margins (NOT a uniform `master/COLS × master/ROWS` cell grid).
    fn card_src_origin(row: u32, col: u32) -> (f64, f64) {
        (
            SRC_LEFT_MARGIN + col as f64 * SRC_PITCH_X,
            SRC_TOP_MARGIN + row as f64 * SRC_PITCH_Y,
        )
    }

    /// Render the card at `(row, col)` into a fresh, owned RGBA8 sprite.
    pub fn extract(&self, row: u32, col: u32) -> Vec<u8> {
        debug_assert!(row < ROWS && col < COLS, "({row}, {col}) outside grid");
        let (src_x, src_y) = Self::card_src_origin(row, col);
        let mut out = if self.lcd_subpixel {
            let wide_w = self
                .card_px_w
                .checked_mul(3)
                .expect("LCD wide-buffer width fits in u32");
            let fb = render_svg_tree_region_to_framebuffer_at_size(
                deck_tree(),
                src_x,
                src_y,
                SRC_CARD_W,
                SRC_CARD_H,
                wide_w,
                self.card_px_h,
            )
            .expect("bundled CC0 card SVG region rasterizes (LCD wide buffer)");
            let mut wide = fb.pixels_flipped();
            unpremultiply_rgba_inplace(&mut wide);
            lcd_subpixel_collapse(&wide, wide_w, self.card_px_h, self.card_px_w)
        } else {
            let fb = render_svg_tree_region_to_framebuffer_at_size(
                deck_tree(),
                src_x,
                src_y,
                SRC_CARD_W,
                SRC_CARD_H,
                self.card_px_w,
                self.card_px_h,
            )
            .expect("bundled CC0 card SVG region rasterizes");
            let mut pixels = fb.pixels_flipped();
            unpremultiply_rgba_inplace(&mut pixels);
            pixels
        };

        debug_assert_eq!(out.len(), (self.card_px_w * self.card_px_h * 4) as usize);
        paint_one_pixel_card_border(&mut out, self.card_px_w, self.card_px_h);
        out
    }

    pub fn extract_face(&self, suit: Suit, rank: Rank) -> Vec<u8> {
        let (row, col) = position_for(suit, rank);
        self.extract(row, col)
    }

    /// The default card back is the intricate blue interlace pattern
    /// in the ancillary row at column 4. The simpler rounded-rectangle
    /// at row 4 / col 0 is still there in the master if a future deck
    /// theme wants a minimalist look.
    pub fn extract_back(&self) -> Vec<u8> {
        self.extract(4, 4)
    }
}

/// Final pixel-space card outline pass. The SVG deck is rasterized at many
/// output sizes; relying on the source outline means rounding/cropping can
/// occasionally drop an edge pixel. This pass finds the alpha silhouette of
/// the cropped card and paints the inside boundary black, giving every sprite
/// a stable 1-device-pixel outline at its actual atlas size.
fn paint_one_pixel_card_border(pixels: &mut [u8], w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    debug_assert_eq!(pixels.len(), (w * h * 4) as usize);
    const ALPHA_THRESHOLD: u8 = 16;
    let src = pixels.to_vec();
    let idx = |x: u32, y: u32| -> usize { ((y * w + x) * 4) as usize };
    let belongs_to_card = |x: u32, y: u32| -> bool { src[idx(x, y) + 3] >= ALPHA_THRESHOLD };

    for y in 0..h {
        for x in 0..w {
            if !belongs_to_card(x, y) {
                continue;
            }
            let mut boundary = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            if !boundary {
                let x0 = x.saturating_sub(1);
                let y0 = y.saturating_sub(1);
                let x1 = (x + 1).min(w - 1);
                let y1 = (y + 1).min(h - 1);
                'neighbors: for ny in y0..=y1 {
                    for nx in x0..=x1 {
                        if nx == x && ny == y {
                            continue;
                        }
                        if !belongs_to_card(nx, ny) {
                            boundary = true;
                            break 'neighbors;
                        }
                    }
                }
            }
            if boundary {
                let i = idx(x, y);
                pixels[i] = 0;
                pixels[i + 1] = 0;
                pixels[i + 2] = 0;
                pixels[i + 3] = src[i + 3].max(0xcc);
            }
        }
    }
}

/// Collapse a 3×-horizontal RGBA "LCD back buffer" into a normal-width
/// RGBA8 image using per-channel offset sampling through a 5-tap
/// `[1, 4, 7, 4, 1] / 17` low-pass filter. For each output pixel
/// column `x`:
///
/// - output `R` = filter centred at source subcolumn `3x + 0`, sampling the source R channel
/// - output `G` = filter centred at source subcolumn `3x + 1`, sampling the source G channel
/// - output `B` = filter centred at source subcolumn `3x + 2`, sampling the source B channel
/// - output `A` = filter centred at the pixel centre (subcolumn `3x + 1`), sampling the source A channel
///
/// On RGB-stripe LCDs, this lights up each physical R/G/B subpixel from
/// a different sub-column of the wide source — perceptually around 3×
/// the horizontal resolution of a same-pixel-count plain raster, at
/// the cost of mild chroma fringing on saturated edges. The 5-tap
/// low-pass dampens the worst fringing while keeping the resolution
/// boost.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn lcd_subpixel_collapse(src: &[u8], src_w: u32, h: u32, out_w: u32) -> Vec<u8> {
    debug_assert_eq!(src_w, out_w * 3);
    debug_assert_eq!(src.len(), (src_w as usize) * (h as usize) * 4);
    let filter = [1u32, 4, 7, 4, 1];
    let filter_sum: u32 = 17;
    let src_w_i = src_w as i32;
    let src_stride = (src_w as usize) * 4;
    let dst_stride = (out_w as usize) * 4;
    let mut out = vec![0u8; dst_stride * (h as usize)];

    let sample = |row_off: usize, sub: i32, channel: usize| -> u32 {
        let s = sub.clamp(0, src_w_i - 1) as usize;
        src[row_off + s * 4 + channel] as u32
    };

    for y in 0..h as usize {
        let src_row = y * src_stride;
        let dst_row = y * dst_stride;
        for x in 0..out_w as i32 {
            let tgt_r = 3 * x;
            let tgt_g = 3 * x + 1;
            let tgt_b = 3 * x + 2;
            let mut acc_r = 0u32;
            let mut acc_g = 0u32;
            let mut acc_b = 0u32;
            let mut acc_a = 0u32;
            for (i, &w) in filter.iter().enumerate() {
                let off = i as i32 - 2;
                acc_r += w * sample(src_row, tgt_r + off, 0);
                acc_g += w * sample(src_row, tgt_g + off, 1);
                acc_b += w * sample(src_row, tgt_b + off, 2);
                // Alpha collapsed through a centred filter so it stays
                // a single per-pixel value compatible with the standard
                // `SRC_ALPHA / 1-SRC_ALPHA` blit path.
                acc_a += w * sample(src_row, tgt_g + off, 3);
            }
            let half = filter_sum / 2;
            let di = dst_row + (x as usize) * 4;
            out[di] = ((acc_r + half) / filter_sum).min(255) as u8;
            out[di + 1] = ((acc_g + half) / filter_sum).min(255) as u8;
            out[di + 2] = ((acc_b + half) / filter_sum).min(255) as u8;
            out[di + 3] = ((acc_a + half) / filter_sum).min(255) as u8;
        }
    }
    out
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

    /// A uniform-colour source must collapse to the same colour with
    /// zero subpixel fringing — the filter is normalised to sum 17, so
    /// 17 identical taps round-trip exactly.
    #[test]
    fn lcd_collapse_uniform_color_is_unchanged() {
        let out_w = 4u32;
        let src_w = out_w * 3;
        let h = 2u32;
        // Solid opaque slate-blue.
        let mut src = Vec::with_capacity((src_w * h * 4) as usize);
        for _ in 0..src_w * h {
            src.extend_from_slice(&[0x33, 0x55, 0x99, 0xff]);
        }
        let out = lcd_subpixel_collapse(&src, src_w, h, out_w);
        for px in out.chunks_exact(4) {
            assert_eq!(px, [0x33, 0x55, 0x99, 0xff]);
        }
    }

    /// A hard white-to-black edge crossing the centre of the output
    /// must produce a per-channel staircase in the output pixel that
    /// straddles the edge: R (left subpixel) brightest, B (right
    /// subpixel) darkest. That ordering is the whole point of the
    /// LCD-RGB path.
    #[test]
    fn lcd_collapse_white_to_black_edge_has_rgb_staircase() {
        let out_w = 4u32;
        let src_w = out_w * 3; // = 12
        let h = 1u32;
        // White for the first 6 subcolumns, black for the last 6.
        // So the centre of the edge sits between subcolumns 5 and 6,
        // i.e. inside output pixel x = 2 (which spans subcolumns 6/7/8).
        let mut src = Vec::with_capacity((src_w * 4) as usize);
        for sx in 0..src_w {
            if sx < src_w / 2 {
                src.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            } else {
                src.extend_from_slice(&[0x00, 0x00, 0x00, 0xff]);
            }
        }
        let out = lcd_subpixel_collapse(&src, src_w, h, out_w);
        // Output pixel 2 straddles the edge; R subpixel sits on
        // subcolumn 6 (just past the edge so a little white leaks in via
        // the 5-tap filter), G on 7, B on 8 — each strictly darker.
        let r = out[2 * 4];
        let g = out[2 * 4 + 1];
        let b = out[2 * 4 + 2];
        assert!(
            r > g && g > b,
            "edge pixel should be R>G>B but got R={r} G={g} B={b}",
        );
        // Pixels left of the edge stay white, right of the edge stay black.
        assert_eq!(&out[0..3], &[0xff, 0xff, 0xff]);
        assert_eq!(
            &out[(out_w as usize - 1) * 4..(out_w as usize - 1) * 4 + 3],
            &[0, 0, 0]
        );
    }

    #[test]
    fn position_corners() {
        assert_eq!(position_for(Suit::Spades, Rank::Ace), (0, 0));
        assert_eq!(position_for(Suit::Spades, Rank::King), (0, 12));
        assert_eq!(position_for(Suit::Clubs, Rank::Ace), (3, 0));
        assert_eq!(position_for(Suit::Clubs, Rank::King), (3, 12));
        assert_eq!(position_for(Suit::Diamonds, Rank::Seven), (2, 6));
    }

    #[test]
    fn card_border_pass_paints_alpha_silhouette_boundary() {
        let w = 5;
        let h = 5;
        let mut pixels = vec![0u8; w * h * 4];
        for y in 1..4 {
            for x in 1..4 {
                let i = (y * w + x) * 4;
                pixels[i] = 0xff;
                pixels[i + 1] = 0xff;
                pixels[i + 2] = 0xff;
                pixels[i + 3] = 0xff;
            }
        }

        paint_one_pixel_card_border(&mut pixels, w as u32, h as u32);

        let center = (2 * w + 2) * 4;
        assert_eq!(&pixels[center..center + 4], &[0xff, 0xff, 0xff, 0xff]);
        for &(x, y) in &[
            (1usize, 1usize),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ] {
            let i = (y * w + x) * 4;
            assert_eq!(&pixels[i..i + 4], &[0, 0, 0, 0xff]);
        }
    }

    /// Sanity check that the bundled SVG parses and rasterizes at a
    /// small size, and that face/back extraction returns the expected
    /// byte counts. Slow (parses 7.6 MB of XML) but a one-time guard
    /// against the asset getting corrupted or usvg dropping support.
    #[test]
    #[ignore = "parses the 7.6 MB CC0 deck — opt in via `--ignored`"]
    fn deck_rasterizes_and_extracts() {
        let bmp = DeckBitmap::build(20, 28);
        let face = bmp.extract_face(Suit::Hearts, Rank::Queen);
        assert_eq!(face.len(), (20 * 28 * 4) as usize);
        let back = bmp.extract_back();
        assert_eq!(back.len(), (20 * 28 * 4) as usize);
        assert_ne!(face, back);
    }

    #[test]
    #[ignore = "parses the 7.6 MB CC0 deck — opt in via `--ignored`"]
    fn face_sprite_contains_face_art() {
        let bmp = DeckBitmap::build(90, 126);
        let face = bmp.extract_face(Suit::Hearts, Rank::Queen);
        let back = bmp.extract_back();
        assert_ne!(face, back);

        let face_whiteish = face
            .chunks_exact(4)
            .filter(|px| px[3] > 200 && px[0] > 220 && px[1] > 220 && px[2] > 220)
            .count();
        let face_redish = face
            .chunks_exact(4)
            .filter(|px| px[3] > 200 && px[0] > 120 && px[1] < 80 && px[2] < 80)
            .count();
        let back_whiteish = back
            .chunks_exact(4)
            .filter(|px| px[3] > 200 && px[0] > 220 && px[1] > 220 && px[2] > 220)
            .count();
        assert!(face_whiteish > 1000, "face should contain white card body");
        assert!(face_redish > 20, "heart face should contain red art");
        assert!(
            face_whiteish > back_whiteish * 2,
            "face should be much whiter than the blue card back"
        );
    }

    /// Diagnostic: rasterize the master at source dims (≈5109×2883) and
    /// for each grid cell print the alpha-bounding box, expressed as
    /// fractions of the master, so we can pin down where each card
    /// actually lives inside its cell. Run via:
    ///   cargo test deck_layout_probe -p solitaire-core -- --ignored --nocapture
    #[test]
    #[ignore = "diagnostic only — prints per-cell card bounds"]
    fn deck_layout_probe() {
        let card_px_w = 393u32;
        let card_px_h = 577u32;
        let bmp = DeckBitmap::build(card_px_w, card_px_h);
        let stride = card_px_w * 4;

        for r in 0..ROWS {
            for c in 0..COLS {
                let pixels = bmp.extract(r, c);
                let mut min_x = u32::MAX;
                let mut max_x = 0u32;
                let mut min_y = u32::MAX;
                let mut max_y = 0u32;
                for y in 0..card_px_h {
                    for x in 0..card_px_w {
                        let alpha = pixels[(y * stride + x * 4 + 3) as usize];
                        if alpha > 200 {
                            if x < min_x {
                                min_x = x;
                            }
                            if x > max_x {
                                max_x = x;
                            }
                            if y < min_y {
                                min_y = y;
                            }
                            if y > max_y {
                                max_y = y;
                            }
                        }
                    }
                }
                if min_x == u32::MAX {
                    println!("r={r} c={c}: EMPTY CELL");
                } else {
                    let lx = min_x as f64 / card_px_w as f64;
                    let rx = (max_x + 1) as f64 / card_px_w as f64;
                    let ty = min_y as f64 / card_px_h as f64;
                    let by = (max_y + 1) as f64 / card_px_h as f64;
                    println!(
                        "r={r} c={c}: card={card_px_w}x{card_px_h}  bbox=[{lx:.3},{ty:.3}..{rx:.3},{by:.3}]"
                    );
                }
            }
        }
    }
}
