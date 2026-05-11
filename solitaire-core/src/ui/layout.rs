//! Chrome layout — picks where the menu bar, HUD, and playfield
//! sit based on the current viewport size. On wide / tall desktops
//! the menu bar runs across the top, the HUD along the bottom, and
//! the playfield fills the middle. On landscape-mobile-shaped
//! viewports (short / wide) the HUD switches to a vertical strip on
//! the LEFT so the playfield gets the full window height for cards.
//!
//! Every chrome widget reads its own `Rect` out of `compute()` —
//! `OverlayStack` still hands every child the full viewport bounds,
//! and each widget restricts its paint + hit-test to its slice.

use agg_gui::geometry::{Rect, Size};

/// Width of the left-side chrome column in `Sidebar` mode (logical px).
/// Sized for a 44×44-ish tap target plus padding.
pub const SIDEBAR_W: f64 = 120.0;
/// Menu-bar height matching `agg_gui::widgets::menu::MENU_BAR_H`.
/// Hard-coded here to keep this module dependency-free for tests.
pub const MENU_BAR_H: f64 = 26.0;
/// HUD strip height in `Standard` mode.
pub const HUD_STRIP_H: f64 = 48.0;
/// Breathing room between the playfield and the chrome (menu bar, HUD
/// strip, or sidebar) so cards never paint flush against a chrome
/// edge. Applied symmetrically on every side the playfield touches
/// chrome.
pub const PLAY_PAD: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeMode {
    /// Menu bar across the top, HUD strip across the bottom. The
    /// default for desktop and portrait phone aspects.
    Standard,
    /// Menu bar at the top, HUD as a vertical column on the LEFT.
    /// Used when the viewport is short enough that bottom-strip
    /// HUD would eat too much vertical room — typically a phone in
    /// landscape.
    Sidebar,
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

/// Decide the chrome layout for the given viewport. The thresholds
/// are intentionally generous so a slightly cramped desktop window
/// keeps the familiar bottom-strip layout.
pub fn compute(viewport: Size) -> ChromeLayout {
    let w = viewport.width;
    let h = viewport.height;
    // Compact when the window is short enough that a 48-px bottom
    // strip + 26-px top strip would eat >15% of vertical room, or
    // when the aspect is markedly landscape-wide (phones in
    // landscape, 16:9 / 19.5:9). Either triggers sidebar mode.
    let compact = h < 700.0 || (w > h * 1.5 && h < 900.0);
    if compact {
        // Sidebar mode: NO top menu bar — the menu actions get exposed
        // as vertical buttons inside the left sidebar instead, so the
        // playfield can reach all the way to the top of the viewport
        // and cards are as large as possible. `menu_rect` is reported
        // as zero-area so the chrome consumer can ignore it.
        let menu_rect = Rect::new(0.0, 0.0, 0.0, 0.0);
        let hud_rect = Rect::new(0.0, 0.0, SIDEBAR_W, h);
        // Tight inset on every side so the playfield gets the
        // overwhelming majority of the screen in landscape mobile.
        const SIDEBAR_PAD: f64 = 6.0;
        let playfield_rect = Rect::new(
            SIDEBAR_W + SIDEBAR_PAD,
            SIDEBAR_PAD,
            w - SIDEBAR_W - SIDEBAR_PAD * 2.0,
            h - SIDEBAR_PAD * 2.0,
        );
        ChromeLayout {
            mode: ChromeMode::Sidebar,
            menu_rect,
            hud_rect,
            playfield_rect,
        }
    } else {
        let menu_rect = Rect::new(0.0, h - MENU_BAR_H, w, MENU_BAR_H);
        let hud_rect = Rect::new(0.0, 0.0, w, HUD_STRIP_H);
        // Inset top + bottom so cards have breathing room between the
        // menu bar above and HUD strip below.
        let playfield_rect = Rect::new(
            0.0,
            HUD_STRIP_H + PLAY_PAD,
            w,
            h - HUD_STRIP_H - MENU_BAR_H - PLAY_PAD * 2.0,
        );
        ChromeLayout {
            mode: ChromeMode::Standard,
            menu_rect,
            hud_rect,
            playfield_rect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_uses_standard_layout() {
        let l = compute(Size::new(1024.0, 720.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        // Playfield grabs the middle band minus PLAY_PAD breathing room
        // between menu/HUD and the cards.
        assert_eq!(l.playfield_rect.width, 1024.0);
        assert_eq!(
            l.playfield_rect.height,
            720.0 - MENU_BAR_H - HUD_STRIP_H - PLAY_PAD * 2.0
        );
        assert_eq!(l.playfield_rect.y, HUD_STRIP_H + PLAY_PAD);
    }

    #[test]
    fn landscape_phone_switches_to_sidebar() {
        // ~iPhone in landscape (logical pixels): 844 × 390.
        let l = compute(Size::new(844.0, 390.0));
        assert_eq!(l.mode, ChromeMode::Sidebar);
        // Sidebar runs the full viewport height — no menu bar in this
        // mode, the menu actions are exposed as sidebar buttons.
        assert_eq!(l.hud_rect.width, SIDEBAR_W);
        assert_eq!(l.hud_rect.height, 390.0);
        // No menu bar painted in sidebar mode.
        assert_eq!(l.menu_rect.width, 0.0);
        assert_eq!(l.menu_rect.height, 0.0);
        // Playfield sits to the right of the sidebar with a tight pad.
        assert!(l.playfield_rect.x > SIDEBAR_W);
        assert!(l.playfield_rect.width > 844.0 - SIDEBAR_W - 30.0);
    }

    #[test]
    fn portrait_phone_stays_in_standard_layout() {
        // Portrait phone is tall — current rules keep the standard
        // top-menu + bottom-HUD layout. (A separate "portrait"
        // mode is a future option.)
        let l = compute(Size::new(390.0, 844.0));
        assert_eq!(l.mode, ChromeMode::Standard);
    }
}
