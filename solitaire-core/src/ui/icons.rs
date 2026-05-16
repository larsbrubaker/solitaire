//! Font Awesome icon code points used by the Solitaire UI.
//!
//! Per CLAUDE.md "Icons: Font Awesome via Unicode code points": every UI
//! icon renders through a Font Awesome font face, never as a raster
//! asset. This module centralises the code-point constants so widgets
//! never sprinkle raw `'\u{...}'` literals.
//!
//! The FA fonts themselves are not yet bundled — Phase 2's UI uses text
//! labels. When Phase 1's polish pass adds icon buttons, bundle FA Free
//! Solid and FA Free Brands TTFs under `solitaire-core/assets/` and load
//! them as additional `Arc<Font>`s alongside the default text font.

#![allow(dead_code)]

/// FA Free Solid: f013 — gear / settings.
pub const FA_GEAR: char = '\u{f013}';
/// FA Free Solid: f01e — undo / rotate-back.
pub const FA_UNDO: char = '\u{f01e}';
/// FA Free Solid: f021 — refresh / arrows-rotate (new deal).
pub const FA_REFRESH: char = '\u{f021}';
/// FA Free Solid: f015 — house (back to title).
pub const FA_HOME: char = '\u{f015}';
/// FA Free Solid: f091 — trophy (leaderboard / win).
pub const FA_TROPHY: char = '\u{f091}';
/// FA Free Solid: f00d — x-mark (cancel / close).
pub const FA_XMARK: char = '\u{f00d}';
/// FA Free Solid: f04b — play (filled triangle).
pub const FA_PLAY: char = '\u{f04b}';
/// FA Free Solid: f0eb — lightbulb (hint).
pub const FA_LIGHTBULB: char = '\u{f0eb}';
/// FA Free Solid: f065 — expand (fullscreen).
pub const FA_EXPAND: char = '\u{f065}';
/// FA Free Solid: f0c9 — bars (hamburger menu).
pub const FA_BARS: char = '\u{f0c9}';
