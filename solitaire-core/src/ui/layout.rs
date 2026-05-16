//! Chrome layout — picks where the menu bar, HUD, and playfield
//! sit based on the current viewport size. The menu button and
//! the HUD action-button row share a SINGLE strip across the TOP
//! of the viewport: the "Menu" cascade button sits in a left-side
//! sliver and the action buttons centre-align across the
//! remaining width. The playfield fills the entire area below
//! the strip.
//!
//! Every chrome widget reads its own `Rect` out of `compute()` —
//! `OverlayStack` still hands every child the full viewport
//! bounds, and each widget restricts its paint + hit-test to its
//! slice.

use agg_gui::geometry::{Rect, Size};

/// Inner menu-bar height matching `agg_gui::widgets::menu::MENU_BAR_H`.
/// Hard-coded here to keep this module dependency-free for tests.
pub const MENU_BAR_H: f64 = 26.0;
/// Combined chrome strip height — single row hosting both the
/// menu cascade button (left) and the HUD action buttons (right).
pub const HUD_STRIP_H: f64 = 48.0;
/// Width reserved at the LEFT of the chrome strip for the "Menu"
/// cascade button. Sized to fit the label + padding + a little
/// gutter so the action buttons start with a clean margin.
pub const MENU_AREA_W: f64 = 110.0;
/// Breathing room between the playfield and the chrome/window edges so
/// cards never paint flush against the UI frame.
pub const PLAY_PAD: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeMode {
    /// Menu bar + HUD strip stacked at the bottom of the viewport;
    /// playfield gets everything above. The only mode we ship —
    /// `ChromeMode` is kept as an enum for forward-compat
    /// (alternative chrome layouts can be added later) but every
    /// `compute()` result is `Standard` for now.
    Standard,
}

#[derive(Clone, Copy, Debug)]
pub struct ChromeLayout {
    pub mode: ChromeMode,
    /// Rect where the menu bar paints (Y-up logical coords, origin
    /// at the bottom-left of the viewport).
    pub menu_rect: Rect,
    /// Rect where the HUD paints. Horizontal strip in Standard;
    /// vertical column in Sidebar.
    pub hud_rect: Rect,
    /// Rect available to the game playfield. The game's
    /// `playfield_transform` letterboxes 1024×720 inside this.
    pub playfield_rect: Rect,
}

/// Decide the chrome layout for the given viewport. Returns a
/// single top-of-viewport strip split horizontally: the left
/// `MENU_AREA_W` pixels host the "Menu" cascade button, the rest
/// of the strip is the HUD action-button row. The playfield
/// inherits everything below the strip.
pub fn compute(viewport: Size) -> ChromeLayout {
    let w = viewport.width;
    let h = viewport.height;
    let strip_h = HUD_STRIP_H;
    // Y-up: top of the viewport is at y = h. Strip occupies
    // [h - strip_h, h].
    let strip_y = (h - strip_h).max(0.0);
    let menu_w = MENU_AREA_W.min(w);
    let menu_rect = Rect::new(0.0, strip_y, menu_w, strip_h);
    let hud_rect = Rect::new(menu_w, strip_y, (w - menu_w).max(0.0), strip_h);
    // Playfield occupies the rest of the viewport (below the
    // chrome strip in Y-up), inset by PLAY_PAD so cards have
    // breathing room.
    let playfield_rect = Rect::new(
        PLAY_PAD,
        PLAY_PAD,
        (w - PLAY_PAD * 2.0).max(0.0),
        (h - strip_h - PLAY_PAD * 2.0).max(0.0),
    );
    ChromeLayout {
        mode: ChromeMode::Standard,
        menu_rect,
        hud_rect,
        playfield_rect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_uses_standard_layout() {
        let l = compute(Size::new(1024.0, 720.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        // Menu and HUD share one strip at the TOP of the viewport
        // (Y-up: high y near `viewport.height`). Menu takes the
        // left `MENU_AREA_W` pixels; HUD takes the remainder.
        let strip_y = 720.0 - HUD_STRIP_H;
        assert_eq!(l.menu_rect.y, strip_y);
        assert_eq!(l.menu_rect.x, 0.0);
        assert_eq!(l.menu_rect.width, MENU_AREA_W);
        assert_eq!(l.menu_rect.height, HUD_STRIP_H);
        assert_eq!(l.hud_rect.y, strip_y);
        assert_eq!(l.hud_rect.x, MENU_AREA_W);
        assert_eq!(l.hud_rect.width, 1024.0 - MENU_AREA_W);
        assert_eq!(l.hud_rect.height, HUD_STRIP_H);
        // Playfield fills everything below the strip.
        assert_eq!(l.playfield_rect.x, PLAY_PAD);
        assert_eq!(l.playfield_rect.y, PLAY_PAD);
        assert_eq!(l.playfield_rect.width, 1024.0 - PLAY_PAD * 2.0);
        assert_eq!(
            l.playfield_rect.height,
            720.0 - HUD_STRIP_H - PLAY_PAD * 2.0
        );
    }

    #[test]
    fn landscape_phone_uses_top_strip() {
        // ~iPhone in landscape (logical pixels): 844 × 390. Same
        // top-strip layout as desktop — menu on the left, HUD
        // action buttons on the right, playfield below.
        let l = compute(Size::new(844.0, 390.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        assert_eq!(l.menu_rect.y, 390.0 - HUD_STRIP_H);
        assert_eq!(l.hud_rect.y, 390.0 - HUD_STRIP_H);
        assert!(l.playfield_rect.height > 0.0);
    }

    #[test]
    fn portrait_phone_stays_in_standard_layout() {
        // Portrait phone is tall — the top-strip layout still
        // applies. Playfield gets all the vertical room it can.
        let l = compute(Size::new(390.0, 844.0));
        assert_eq!(l.mode, ChromeMode::Standard);
    }

    #[test]
    fn cramped_non_landscape_window_stays_standard() {
        let l = compute(Size::new(654.0, 690.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        assert_eq!(l.menu_rect.y, 690.0 - HUD_STRIP_H);
        assert_eq!(l.hud_rect.y, 690.0 - HUD_STRIP_H);
        assert_eq!(l.playfield_rect.x, PLAY_PAD);
        assert_eq!(l.playfield_rect.width, 654.0 - PLAY_PAD * 2.0);
    }
}
