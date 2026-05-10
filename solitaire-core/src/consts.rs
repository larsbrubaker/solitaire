//! Layout constants for the Solitaire playfield.
//!
//! All coordinates are Y-up first-quadrant (origin bottom-left), consistent
//! with agg-gui's coordinate convention.

/// Virtual playfield width. The shells letterbox/scale to fit the window,
/// so card-positioning math always works in this fixed coordinate space.
pub const VIRTUAL_W: f64 = 1024.0;
pub const VIRTUAL_H: f64 = 720.0;

pub const CARD_W: f64 = 90.0;
pub const CARD_H: f64 = 126.0;
pub const CARD_CORNER_R: f64 = 8.0;

/// Horizontal gap between adjacent cards / columns.
pub const COL_GAP_X: f64 = 26.0;
/// Pixel column-pitch (card width + gap).
pub const COL_PITCH: f64 = CARD_W + COL_GAP_X;

/// Vertical gap between the top row (stock/waste/foundations) and the
/// tableau row.
pub const ROW_GAP_Y: f64 = 28.0;

/// Top margin of the playfield (distance from the top of the playfield to
/// the top edge of the top row of cards, in Y-down terms).
pub const TOP_MARGIN: f64 = 24.0;

/// Reserved space at the bottom for the HUD strip.
pub const HUD_HEIGHT: f64 = 48.0;

/// Fan offset between successive face-up cards in a tableau column.
pub const TABLEAU_FAN_DOWN: f64 = 28.0;
/// Fan offset between successive face-down cards in a tableau column.
/// Smaller than face-up because nothing readable is shown on the back.
pub const TABLEAU_FAN_DOWN_FACEDOWN: f64 = 14.0;

/// X-offset between cards in the waste-pile fan when Klondike runs in
/// 3-card-draw mode (Microsoft "Classic" style — the most-recent draw
/// stays visible). The empty column to the right of the waste leaves
/// room for ~2 × COL_PITCH of fan width before hitting the foundations.
pub const WASTE_FAN_DX: f64 = 24.0;

/// Y-up origin (bottom of card) for the top row of cards (stock, waste,
/// foundations).
pub const TOP_ROW_BOTTOM_Y: f64 = VIRTUAL_H - TOP_MARGIN - CARD_H;

/// Y-up origin for the BASE card (card index 0) of every tableau column.
/// Subsequent cards fan downward in screen terms = smaller numerical Y.
pub const TABLEAU_BASE_Y: f64 = TOP_ROW_BOTTOM_Y - CARD_H - ROW_GAP_Y;

/// Computed left margin so 7 tableau columns center horizontally.
pub const PLAYFIELD_LEFT: f64 = (VIRTUAL_W - (6.0 * COL_PITCH + CARD_W)) / 2.0;
