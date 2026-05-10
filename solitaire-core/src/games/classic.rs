//! Classic — Microsoft-style Klondike variant ported from
//! [DualBrain/Solitaire](https://github.com/DualBrain/Solitaire). Same
//! tableau / foundations layout as Klondike; the only ruleset difference
//! is **3-card draw** from the stock instead of 1-card.
//!
//! Implementation: re-uses `Klondike` with `draw_count = 3` and a
//! distinct `slug` so leaderboard scores stay separated from
//! standard-Klondike scores.

use super::klondike::Klondike;

pub fn rules() -> Klondike {
    Klondike::classic()
}
