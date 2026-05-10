//! Procedural card rendering — no PNGs, no SVG. Every card face, suit
//! glyph, and pip layout is drawn with `DrawCtx` primitives at paint time.

mod card_back;
mod card_face;
mod pile_paint;

pub use card_back::paint_card_back;
pub use card_face::paint_card_face;
pub use pile_paint::{paint_empty_slot, paint_pile};

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
