//! Chrome layout — picks where the menu bar, HUD, and playfield
//! sit based on the current viewport size. Both the menu bar and
//! the HUD action-button strip stack across the BOTTOM of the
//! viewport for every viewport shape (no more sidebar mode). The
//! playfield fills the entire area above the combined chrome.
//!
//! Every chrome widget reads its own `Rect` out of `compute()` —
//! `OverlayStack` still hands every child the full viewport
//! bounds, and each widget restricts its paint + hit-test to its
//! slice.

use agg_gui::geometry::{Rect, Size};

/// Menu-bar height matching `agg_gui::widgets::menu::MENU_BAR_H`.
/// Hard-coded here to keep this module dependency-free for tests.
pub const MENU_BAR_H: f64 = 26.0;
/// HUD strip height — sits immediately above the menu bar.
pub const HUD_STRIP_H: f64 = 48.0;
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

/// Decide the chrome layout for the given viewport. Always
/// returns the bottom-stacked Standard layout — every viewport
/// shape (desktop, portrait phone, landscape phone) uses the same
/// chrome arrangement so the player's muscle memory transfers
/// across devices.
pub fn compute(viewport: Size) -> ChromeLayout {
    let w = viewport.width;
    let h = viewport.height;
    // Both menu bar and HUD stack across the BOTTOM of the
    // viewport (Y-up: smaller y = lower on screen). Menu sits at
    // y=0 closest to the thumb; HUD action buttons stack
    // immediately above the menu. Playfield gets the rest of the
    // viewport above the combined chrome.
    let menu_rect = Rect::new(0.0, 0.0, w, MENU_BAR_H);
    let hud_rect = Rect::new(0.0, MENU_BAR_H, w, HUD_STRIP_H);
    let chrome_h = MENU_BAR_H + HUD_STRIP_H;
    let playfield_rect = Rect::new(
        PLAY_PAD,
        chrome_h + PLAY_PAD,
        w - PLAY_PAD * 2.0,
        h - chrome_h - PLAY_PAD * 2.0,
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
        // Menu + HUD stack at the bottom (Y-up): menu at y=0,
        // HUD just above it, playfield above the combined chrome.
        assert_eq!(l.menu_rect.y, 0.0);
        assert_eq!(l.menu_rect.height, MENU_BAR_H);
        assert_eq!(l.hud_rect.y, MENU_BAR_H);
        assert_eq!(l.hud_rect.height, HUD_STRIP_H);
        assert_eq!(l.playfield_rect.x, PLAY_PAD);
        assert_eq!(l.playfield_rect.width, 1024.0 - PLAY_PAD * 2.0);
        assert_eq!(
            l.playfield_rect.height,
            720.0 - MENU_BAR_H - HUD_STRIP_H - PLAY_PAD * 2.0
        );
        assert_eq!(l.playfield_rect.y, MENU_BAR_H + HUD_STRIP_H + PLAY_PAD);
    }

    #[test]
    fn landscape_phone_uses_bottom_strip() {
        // ~iPhone in landscape (logical pixels): 844 × 390. Used
        // to switch to a sidebar mode; now the same bottom-strip
        // layout applies everywhere so the player's muscle memory
        // transfers across devices.
        let l = compute(Size::new(844.0, 390.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        assert_eq!(l.menu_rect.y, 0.0);
        assert_eq!(l.hud_rect.y, MENU_BAR_H);
        assert!(l.playfield_rect.height > 0.0);
    }

    #[test]
    fn portrait_phone_stays_in_standard_layout() {
        // Portrait phone is tall — current rules keep the standard
        // top-menu + bottom-HUD layout. (A separate "portrait"
        // mode is a future option.)
        let l = compute(Size::new(390.0, 844.0));
        assert_eq!(l.mode, ChromeMode::Standard);
    }

    #[test]
    fn cramped_non_landscape_window_stays_standard() {
        // This shape needs horizontal room for wide games like Spider;
        // sidebar mode would spend too much width on chrome.
        let l = compute(Size::new(654.0, 690.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        assert_eq!(l.menu_rect.y, 0.0);
        assert_eq!(l.hud_rect.y, MENU_BAR_H);
        assert_eq!(l.playfield_rect.x, PLAY_PAD);
        assert_eq!(l.playfield_rect.width, 654.0 - PLAY_PAD * 2.0);
    }
}
