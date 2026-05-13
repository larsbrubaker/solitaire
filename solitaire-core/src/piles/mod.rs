//! Pile model — kind, layout, hit-test, geometry.
//!
//! Piles do not implement `agg_gui::Widget`. They are pure data structures
//! that a single `GameWidget` paints and hit-tests directly. See
//! `CLAUDE.md` "Drag is owned by GameWidget".

mod hit;
mod layout;
mod pile;
mod set;

#[cfg(test)]
mod tests;

pub use hit::{HitResult, PileSlot};
pub use layout::{PileLayout, FAN_DOWN_FACE_UP};
pub use pile::{Pile, PileKind};
pub use set::PileSet;

/// Index of a pile within a `PileSet`. Stored as `u8` — there are at most
/// 28 piles (Spider has 10 cascades + 8 foundations + 1 stock = 19;
/// Klondike has 13; FreeCell has 16).
pub type PileId = u8;
