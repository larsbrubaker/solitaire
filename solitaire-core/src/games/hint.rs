//! Game-agnostic hint type used by every variant that exposes a
//! Hint button. Variants:
//!
//! - `Move` — pick up `take` cards starting at `start_idx` of the
//!   `from` pile and drop them on the `to` pile. Used by Spider
//!   (cascade → cascade) and Klondike (tableau → tableau / waste
//!   → tableau / tableau → foundation / waste → foundation).
//! - `ClickStock` — click the named stock pile to advance: Spider
//!   deals a row across all cascades, Klondike draws to waste (or
//!   recycles).
//!
//! The `UI`'s rendering doesn't need to care which variant is in
//! play: it highlights `from`/`to` for `Move`, the stock pile for
//! `ClickStock`, and the ghost-card animation kicks off the same
//! way in both cases.

use crate::piles::PileId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Move {
        from: PileId,
        start_idx: usize,
        take: u8,
        to: PileId,
    },
    /// Click the named stock pile to advance. Variant kept name
    /// `StockDeal` for backward compat with earlier Spider-only
    /// API; semantically the same as "click stock to deal" in
    /// both Spider (deals a row across cascades) and Klondike
    /// (draws to waste, or recycles when empty).
    StockDeal { stock: PileId },
}
