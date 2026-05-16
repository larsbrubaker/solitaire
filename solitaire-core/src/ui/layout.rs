//! Chrome layout — picks where the menu bar, HUD, and playfield
//! sit based on the current viewport size. On desktop and portrait-ish
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
/// Height of the vertical Game/Options/Help menu strip pinned to the
/// TOP of the sidebar in `Sidebar` mode. Three top-level menus ×
/// `VERTICAL_ROW_H` (36 px). The HUD action buttons sit below this in
/// the same sidebar column.
pub const SIDEBAR_MENU_H: f64 = 3.0 * 36.0;
/// Breathing room between the playfield and the chrome/window edges so
/// cards never paint flush against the UI frame.
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

/// Decide the chrome layout for the given viewport. Sidebar mode is
/// reserved for genuinely landscape-wide viewports; narrow or roughly
/// square windows need the full horizontal span for cards.
pub fn compute(viewport: Size) -> ChromeLayout {
    let w = viewport.width;
    let h = viewport.height;
    // Compact only when the aspect is markedly landscape-wide (phones
    // in landscape, 16:9 / 19.5:9). A modest desktop window can be
    // short without having horizontal space to spare for a sidebar.
    let compact = w > h * 1.5 && h < 900.0;
    if compact {
        // Sidebar mode:
        //   * `menu_rect` = top portion of the left column — the
        //     vertical Game/Options/Help strip (`SIDEBAR_MENU_H` tall).
        //   * `hud_rect`  = the remainder of the left column — the
        //     action buttons (Undo / New Deal / etc.) stack here.
        //   * `playfield_rect` = the rest of the viewport, inset by a
        //     tight `SIDEBAR_PAD` so cards reach close to the edges.
        let menu_rect = Rect::new(0.0, h - SIDEBAR_MENU_H, SIDEBAR_W, SIDEBAR_MENU_H);
        let hud_rect = Rect::new(0.0, 0.0, SIDEBAR_W, h - SIDEBAR_MENU_H);
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
        // Both menu bar and HUD stack across the BOTTOM of the
        // viewport (Y-up: smaller y = lower on screen). Menu sits
        // at y=0 closest to the thumb; HUD action buttons stack
        // immediately above the menu. Playfield gets the rest of
        // the viewport above the combined chrome.
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
    fn landscape_phone_switches_to_sidebar() {
        // ~iPhone in landscape (logical pixels): 844 × 390.
        let l = compute(Size::new(844.0, 390.0));
        assert_eq!(l.mode, ChromeMode::Sidebar);
        // Sidebar column is split: the top `SIDEBAR_MENU_H` houses the
        // vertical Game/Options/Help strip, the rest below is the HUD
        // action buttons.
        assert_eq!(l.hud_rect.width, SIDEBAR_W);
        assert_eq!(l.hud_rect.height, 390.0 - SIDEBAR_MENU_H);
        assert_eq!(l.menu_rect.width, SIDEBAR_W);
        assert_eq!(l.menu_rect.height, SIDEBAR_MENU_H);
        // Menu sits at the top of the column (high Y in Y-up).
        assert_eq!(l.menu_rect.y, 390.0 - SIDEBAR_MENU_H);
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
