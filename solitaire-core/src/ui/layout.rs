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

/// Width reserved at the LEFT of the chrome strip for the "Menu"
/// cascade button. Sized to fit the label + padding + a little
/// gutter so the action buttons start with a clean margin.
pub const MENU_AREA_W: f64 = 110.0;
/// Breathing room between the playfield and the chrome/window edges so
/// cards never paint flush against the UI frame.
pub const PLAY_PAD: f64 = 12.0;
/// Vertical padding above + below the HUD action buttons inside the
/// combined chrome strip. The strip height derives from the button
/// height plus twice this pad (see [`strip_height`]) so the strip
/// always contains the buttons — on desktop (36 px button) that lands
/// on the historical 48 px strip; on touch (44 px button) it grows.
pub const STRIP_VPAD: f64 = 6.0;

/// Height of a HUD action button in logical px. Floors at the 44 px
/// touch minimum when a touch input profile is active (or a real touch
/// has fired this session), matching agg-gui's menu touch metrics.
/// Queried per layout, never cached: the touch latch can flip at
/// runtime after the first touch.
pub fn hud_button_height() -> f64 {
    if agg_gui::input_profile::touch_ui_active() {
        agg_gui::widgets::menu::TOUCH_MIN
    } else {
        36.0
    }
}

/// Combined chrome strip height — single row hosting both the menu
/// cascade button (left) and the HUD action buttons (right). Derived
/// from the current HUD button height plus vertical padding, and never
/// shorter than the current menu-bar height so the centred menu bar
/// always fits. Query per layout for the same runtime-latch reason as
/// [`hud_button_height`].
pub fn strip_height() -> f64 {
    let content = hud_button_height() + STRIP_VPAD * 2.0;
    content.max(agg_gui::widgets::menu::menu_bar_height())
}

/// Effective width of the left "Menu" sliver, scaled so the label keeps
/// its desktop proportions when the touch menu font grows. The menu bar
/// grows its text by the same ratio its height grows (`menu_bar_height()
/// / BAR_H`): 1.0 on desktop — leaving [`MENU_AREA_W`] at exactly 110 —
/// and >1 once touch sizing is active, so the larger "Menu" label never
/// clips. Query per layout for the same runtime-latch reason as
/// [`strip_height`].
pub fn menu_area_width() -> f64 {
    let ratio = agg_gui::widgets::menu::menu_bar_height() / agg_gui::widgets::menu::MENU_BAR_H;
    MENU_AREA_W * ratio
}

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
    /// Full-width chrome strip spanning the whole top of the viewport
    /// (the union of `menu_rect` + `hud_rect`). This is the ONE
    /// continuous bar the felt-layer painter fills so the menu cascade
    /// button and the HUD action buttons read as a single strip rather
    /// than two mismatched boxes.
    pub strip_rect: Rect,
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
    let strip_h = strip_height();
    // Y-up: top of the viewport is at y = h. Strip occupies
    // [h - strip_h, h].
    let strip_y = (h - strip_h).max(0.0);
    let strip_rect = Rect::new(0.0, strip_y, w, strip_h);
    let menu_w = menu_area_width().min(w);
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
        strip_rect,
        menu_rect,
        hud_rect,
        playfield_rect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agg_gui::input_profile::{set_input_profile, InputProfile};
    use agg_gui::widgets::menu::TOUCH_MIN;

    /// Serialize every test in this module. They all read/write the
    /// process-global input-profile atomic through [`desktop`] / [`mobile`],
    /// so under cargo's parallel test threads the `mobile()` write could be
    /// observed by a sibling desktop-assertion test and flip its expected
    /// sizing. Holding this guard for the full test body keeps at most one
    /// running at a time. (The ideal fix — activating touch via agg-gui's
    /// thread-local `note_touch_event` latch, leaving the global untouched —
    /// is unavailable here: that entry point is `pub(crate)` in agg-gui and
    /// unreachable from this crate. `set_input_profile` is the only public
    /// way to switch touch sizing on, and this crate uses it nowhere but
    /// these tests, so serializing them removes the race entirely.)
    fn profile_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Return every process-global this module reads to a known desktop
    /// baseline. The input profile is a process-wide atomic, so pin it
    /// explicitly; the touch latch and `ux_scale` are thread-local. Mirrors
    /// agg-gui's menu-test `desktop()` helper.
    fn desktop() {
        set_input_profile(InputProfile::Desktop);
        agg_gui::touch_state::clear_last_touch_event_for_testing();
        agg_gui::ux_scale::set_ux_scale(1.0);
    }

    /// Declare a mobile-touch profile so the chrome grows to touch sizing.
    fn mobile() {
        agg_gui::touch_state::clear_last_touch_event_for_testing();
        set_input_profile(InputProfile::MobileIOS);
        agg_gui::ux_scale::set_ux_scale(1.0);
    }

    #[test]
    fn desktop_uses_standard_layout() {
        let _g = profile_guard();
        desktop();
        let strip_h = strip_height();
        // Pin the desktop strip height: 36 px button + 2×6 px pad = 48,
        // the historical constant, and taller than the 26 px menu bar.
        assert_eq!(strip_h, 48.0);
        let l = compute(Size::new(1024.0, 720.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        // Menu and HUD share one strip at the TOP of the viewport
        // (Y-up: high y near `viewport.height`). Menu takes the
        // left `MENU_AREA_W` pixels; HUD takes the remainder.
        let strip_y = 720.0 - strip_h;
        assert_eq!(l.strip_rect.x, 0.0);
        assert_eq!(l.strip_rect.y, strip_y);
        assert_eq!(l.strip_rect.width, 1024.0);
        assert_eq!(l.strip_rect.height, strip_h);
        assert_eq!(l.menu_rect.y, strip_y);
        assert_eq!(l.menu_rect.x, 0.0);
        assert_eq!(l.menu_rect.width, MENU_AREA_W);
        assert_eq!(l.menu_rect.height, strip_h);
        assert_eq!(l.hud_rect.y, strip_y);
        assert_eq!(l.hud_rect.x, MENU_AREA_W);
        assert_eq!(l.hud_rect.width, 1024.0 - MENU_AREA_W);
        assert_eq!(l.hud_rect.height, strip_h);
        // Playfield fills everything below the strip.
        assert_eq!(l.playfield_rect.x, PLAY_PAD);
        assert_eq!(l.playfield_rect.y, PLAY_PAD);
        assert_eq!(l.playfield_rect.width, 1024.0 - PLAY_PAD * 2.0);
        assert_eq!(l.playfield_rect.height, 720.0 - strip_h - PLAY_PAD * 2.0);
        desktop();
    }

    #[test]
    fn touch_profile_grows_strip_and_shrinks_playfield() {
        let _g = profile_guard();
        // Desktop baseline first so the deltas are meaningful.
        desktop();
        // Desktop menu sliver stays pinned at exactly MENU_AREA_W.
        assert_eq!(menu_area_width(), MENU_AREA_W);
        let desk = compute(Size::new(1024.0, 720.0));
        assert_eq!(desk.menu_rect.width, MENU_AREA_W);

        mobile();
        // Buttons floor at the 44 px touch minimum, so the strip grows
        // to contain them: 44 + 2×6 = 56 px.
        assert_eq!(hud_button_height(), TOUCH_MIN);
        assert_eq!(strip_height(), TOUCH_MIN + STRIP_VPAD * 2.0);
        let touch = compute(Size::new(1024.0, 720.0));
        assert!(
            touch.strip_rect.height > desk.strip_rect.height,
            "touch strip must be taller than desktop"
        );
        assert_eq!(touch.strip_rect.width, 1024.0, "strip stays full-width");
        assert_eq!(touch.menu_rect.height, touch.strip_rect.height);
        assert_eq!(touch.hud_rect.height, touch.strip_rect.height);
        // The menu sliver widens with the touch menu font so the larger
        // "Menu" label keeps its proportions instead of clipping.
        assert!(
            menu_area_width() > MENU_AREA_W,
            "touch menu sliver must grow past the desktop width"
        );
        assert!(
            touch.menu_rect.width > desk.menu_rect.width,
            "growing menu sliver must widen the menu rect on touch"
        );
        // The taller chrome eats into the playfield — intended; the
        // pile layout adapts to the smaller rect.
        assert!(
            touch.playfield_rect.height < desk.playfield_rect.height,
            "growing chrome must shrink the playfield on touch"
        );
        desktop();
    }

    #[test]
    fn landscape_phone_uses_top_strip() {
        let _g = profile_guard();
        desktop();
        // ~iPhone in landscape (logical pixels): 844 × 390. Same
        // top-strip layout as desktop — menu on the left, HUD
        // action buttons on the right, playfield below.
        let l = compute(Size::new(844.0, 390.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        let strip_y = 390.0 - strip_height();
        assert_eq!(l.menu_rect.y, strip_y);
        assert_eq!(l.hud_rect.y, strip_y);
        assert!(l.playfield_rect.height > 0.0);
        desktop();
    }

    #[test]
    fn portrait_phone_stays_in_standard_layout() {
        let _g = profile_guard();
        // Portrait phone is tall — the top-strip layout still
        // applies. Playfield gets all the vertical room it can.
        let l = compute(Size::new(390.0, 844.0));
        assert_eq!(l.mode, ChromeMode::Standard);
    }

    #[test]
    fn cramped_non_landscape_window_stays_standard() {
        let _g = profile_guard();
        desktop();
        let strip_y = 690.0 - strip_height();
        let l = compute(Size::new(654.0, 690.0));
        assert_eq!(l.mode, ChromeMode::Standard);
        assert_eq!(l.menu_rect.y, strip_y);
        assert_eq!(l.hud_rect.y, strip_y);
        assert_eq!(l.playfield_rect.x, PLAY_PAD);
        assert_eq!(l.playfield_rect.width, 654.0 - PLAY_PAD * 2.0);
        desktop();
    }
}
