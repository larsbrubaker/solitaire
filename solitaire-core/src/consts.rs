//! Shared rendering constants. Layout coordinates are computed
//! per-game in `GameRules::pile_layout(rect)` and live in screen
//! space; the constants here are visual primitives (card corner
//! radius, etc.) that don't depend on a virtual playfield.

/// Corner radius for card sprites (in screen px relative to default
/// card size). Cards scaled larger or smaller render with the same
/// proportional radius.
pub const CARD_CORNER_R: f64 = 8.0;
