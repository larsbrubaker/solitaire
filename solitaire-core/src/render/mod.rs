//! Card rendering — the active deck is the bundled CC0 SVG ("English
//! pattern playing cards deck PLUS CC0", from Wikimedia Commons),
//! rasterised through agg-gui's SVG renderer when cards are first drawn
//! at a given size.
//! `card_face.rs` and `card_back.rs` retain procedural fallbacks that
//! aren't currently wired into the atlas; keep them around as reference
//! art and as a starting point for future deck themes.

pub mod atlas;
mod card_back;
mod card_face;
mod pile_paint;
mod svg_deck;

pub use atlas::CardSpriteAtlas;
pub use card_back::paint_card_back;
pub use card_face::paint_card_face;
pub use pile_paint::{paint_card_at, paint_empty_slot, paint_pile};

use agg_gui::color::Color;

pub const FELT_GREEN: Color = Color::from_rgb8(0x0c, 0x6b, 0x3a);
pub const FELT_GREEN_DARK: Color = Color::from_rgb8(0x09, 0x52, 0x2c);

pub const CARD_FACE_BG: Color = Color::from_rgb8(0xfa, 0xfa, 0xfa);
pub const CARD_BORDER: Color = Color::from_rgb8(0x10, 0x10, 0x10);
pub const CARD_RED: Color = Color::from_rgb8(0xc0, 0x1c, 0x1c);
pub const CARD_BLACK: Color = Color::from_rgb8(0x12, 0x12, 0x12);

pub const CARD_BACK_BG: Color = Color::from_rgb8(0x16, 0x40, 0x88);
pub const CARD_BACK_PATTERN: Color = Color::from_rgb8(0x29, 0x68, 0xc8);

pub const SLOT_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x40);
pub const HIGHLIGHT: Color = Color::from_rgba8(0xff, 0xd7, 0x00, 0x80);

/// Palette for the win-celebration confetti burst. Festive but
/// card-table-appropriate: suit red, gold, a warm white, plus a bright
/// green and blue so the burst reads against the felt without clashing
/// with the deck art.
pub const CONFETTI_PALETTE: &[Color] = &[
    CARD_RED,
    Color::from_rgb8(0xff, 0xd7, 0x00),
    Color::from_rgb8(0xf5, 0xf3, 0xea),
    Color::from_rgb8(0x2e, 0xc4, 0x6b),
    Color::from_rgb8(0x3a, 0x8d, 0xff),
];

/// Number of flakes spawned when a game is won.
pub const CONFETTI_COUNT: usize = 300;
