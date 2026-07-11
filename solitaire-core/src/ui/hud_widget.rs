//! HUD widget — action buttons (Undo / New Deal / Hint / Shuffle /
//! Main Menu / Full Screen) plus the Mom's-Solitaire shuffle
//! counter. Visible whenever a game is active.
//!
//! Each action is an `agg_gui::widgets::Button` with a Font Awesome
//! glyph painted to the left of its label. The buttons live in
//! `self.children` and the framework's hit-test + paint walk does
//! the heavy lifting — the HUD itself just decides which subset
//! to show for the active variant and positions them as a
//! horizontal strip at the bottom of the viewport (just above
//! the menu bar). When the strip's natural width exceeds the
//! viewport, the buttons collapse to a right-edge hamburger that
//! toggles a vertical popup column.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, Modifiers};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;
use agg_gui::widgets::{Button, ButtonTheme, LabelAlign};

use super::app_model::{Screen, SharedModel};
use super::icons::{FA_BARS, FA_EXPAND, FA_HOME, FA_LIGHTBULB, FA_REFRESH, FA_UNDO};
use super::layout;

const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BG_PRESSED: Color = Color::from_rgb8(0x18, 0x3d, 0x24);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);
const BTN_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const TXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

/// Horizontal gap between HUD action buttons. The button HEIGHT is not
/// a constant — it comes from [`layout::hud_button_height`], which
/// floors at the 44 px touch minimum when a touch profile is active.
const STD_BTN_GAP: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Fullscreen,
    Undo,
    NewDeal,
    Shuffle,
    Hint,
    Home,
    /// Pseudo-action used in compact mode — clicking the
    /// hamburger toggles `model.hud_hamburger_open`. Doesn't
    /// appear in `actions_for()`; only `sync_children` may
    /// inject it at index 0.
    Hamburger,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::Fullscreen => "Full Screen",
            Action::Undo => "Undo",
            Action::NewDeal => "New Deal",
            Action::Shuffle => "Shuffle",
            Action::Hint => "Hint",
            Action::Home => "Main Menu",
            Action::Hamburger => "Menu",
        }
    }

    fn icon(self) -> char {
        match self {
            Action::Fullscreen => FA_EXPAND,
            Action::Undo => FA_UNDO,
            Action::NewDeal => FA_REFRESH,
            // Shuffle reuses the refresh glyph — Mom's variant has
            // no truly unique action so we use the same circular-
            // arrows mark.
            Action::Shuffle => FA_REFRESH,
            Action::Hint => FA_LIGHTBULB,
            Action::Home => FA_HOME,
            Action::Hamburger => FA_BARS,
        }
    }

    fn invoke(self, model: &SharedModel) {
        let mut m = model.borrow_mut();
        match self {
            Action::Fullscreen => {
                drop(m);
                crate::platform::request_toggle_fullscreen();
                return;
            }
            Action::Undo => {
                if let Some(s) = m.session.as_mut() {
                    s.try_undo();
                }
                m.clear_spider_hint();
                m.hud_hamburger_open = false;
            }
            Action::NewDeal => {
                if let Some(kind) = m.kind {
                    m.request_new_deal(kind);
                }
                m.hud_hamburger_open = false;
            }
            Action::Shuffle => {
                m.try_moms_shuffle();
                m.hud_hamburger_open = false;
            }
            Action::Hint => {
                m.show_hint();
                m.hud_hamburger_open = false;
            }
            Action::Home => {
                m.request_main_menu();
                m.hud_hamburger_open = false;
            }
            Action::Hamburger => {
                m.hud_hamburger_open = !m.hud_hamburger_open;
            }
        }
        agg_gui::animation::request_draw();
    }
}

/// Shared HUD button theme — matches the green palette of the
/// menu / dialogs.
fn hud_button_theme() -> ButtonTheme {
    ButtonTheme {
        background: BTN_BG,
        background_hovered: BTN_BG_HOVER,
        background_pressed: BTN_BG_PRESSED,
        label_color: BTN_TEXT,
        border_radius: 8.0,
        focus_ring_color: BTN_BORDER,
        focus_ring_width: 1.5,
    }
}

pub struct HudWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    fa_font: Arc<Font>,
    /// Action assigned to each child button, parallel-indexed.
    /// Rebuilt by `sync_children` when the active variant changes.
    actions: Vec<Action>,
    /// Last variant we built buttons for; used to detect the need
    /// to rebuild the children vec.
    last_actions: Vec<Action>,
}

impl HudWidget {
    pub fn new(model: SharedModel, font: Arc<Font>, fa_font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            fa_font,
            actions: Vec::new(),
            last_actions: Vec::new(),
        }
    }

    /// Buttons to render for the active variant. Mom's Solitaire
    /// gets `Shuffle`; Spider + Klondike get `Hint`; everything
    /// else gets the base four (Fullscreen / Undo / New Deal /
    /// Main Menu).
    fn actions_for(&self) -> Vec<Action> {
        match self.model.borrow().kind {
            Some(crate::games::GameKind::MomsSolitaire) => vec![
                Action::Fullscreen,
                Action::Undo,
                Action::NewDeal,
                Action::Shuffle,
                Action::Home,
            ],
            Some(crate::games::GameKind::Spider) | Some(crate::games::GameKind::Klondike) => vec![
                Action::Fullscreen,
                Action::Undo,
                Action::NewDeal,
                Action::Hint,
                Action::Home,
            ],
            _ => vec![
                Action::Fullscreen,
                Action::Undo,
                Action::NewDeal,
                Action::Home,
            ],
        }
    }

    /// Rebuild `self.children` if `actions_for()` changed since
    /// the last call. Each button captures `self.model` + its
    /// `Action` in its `on_click` closure. The hamburger button
    /// (Action::Hamburger) is always appended at the end — it's
    /// only visible in compact mode, but we keep it built so we
    /// don't have to rebuild the entire vec when the viewport
    /// resizes between wide and narrow.
    fn sync_children(&mut self) {
        // Cache key is the per-variant action list WITHOUT the
        // hamburger; the hamburger is always appended, so we'd
        // otherwise rebuild every layout because `actions_for()`
        // never includes it. (Rebuilding every frame tears down
        // every Button's internal pressed/hovered state, so click
        // capture between MouseDown and MouseUp gets lost.)
        let want = self.actions_for();
        if want == self.last_actions {
            return;
        }
        self.last_actions = want.clone();
        self.children.clear();
        self.actions.clear();
        let font_size: f64 = 16.0;
        let mut all = want;
        all.push(Action::Hamburger);
        for action in &all {
            let model = self.model.clone();
            let a = *action;
            let btn = Button::new(action.label(), self.font.clone())
                .with_font_size(font_size)
                .with_theme(hud_button_theme())
                .with_icon(action.icon(), self.fa_font.clone())
                .with_label_align(LabelAlign::Center)
                .on_click(move || a.invoke(&model));
            self.children.push(Box::new(btn));
            self.actions.push(*action);
        }
    }

    /// Index of the hamburger child (always last after
    /// `sync_children`).
    fn hamburger_idx(&self) -> usize {
        self.children.len().saturating_sub(1)
    }

    fn hamburger_open(&self) -> bool {
        self.model.borrow().hud_hamburger_open
    }

    /// Total horizontal width the per-variant action buttons want
    /// when laid out side by side. Returns the sum of their
    /// natural widths + the gaps between them. Used to decide
    /// whether to switch to compact (hamburger) mode.
    fn measure_action_strip(&mut self) -> f64 {
        let mut total = 0.0;
        let probe = Size::new(self.bounds.width, layout::hud_button_height());
        let action_count = self.children.len().saturating_sub(1);
        for i in 0..action_count {
            let w = self.children[i].layout(probe).width;
            total += w;
        }
        if action_count > 1 {
            total += STD_BTN_GAP * (action_count as f64 - 1.0);
        }
        total
    }

    fn chrome(&self) -> layout::ChromeLayout {
        layout::compute(Size::new(self.bounds.width, self.bounds.height))
    }

    /// Position each button child for the active chrome mode.
    /// Standard: horizontal strip; if too narrow, collapses to a
    /// left-edge hamburger that toggles a vertical popup. Sidebar:
    /// vertical column.
    fn layout_buttons(&mut self) {
        let chrome = self.chrome();
        let hud = chrome.hud_rect;
        let btn_h = layout::hud_button_height();
        let n = self.children.len();
        if n == 0 {
            return;
        }
        let ham = self.hamburger_idx();
        let popup_open = self.hamburger_open();
        let action_count = n.saturating_sub(1);
        let strip_w = self.measure_action_strip();
        let compact = strip_w + STD_BTN_GAP * 2.0 > hud.width;
        if !compact {
            // Wide enough: lay actions in a centered strip and
            // stash the hamburger off-screen so it doesn't paint
            // or accept clicks.
            let probe = Size::new(hud.width, btn_h);
            let widths: Vec<f64> = (0..action_count)
                .map(|i| self.children[i].layout(probe).width)
                .collect();
            let total_w: f64 =
                widths.iter().sum::<f64>() + STD_BTN_GAP * (action_count as f64 - 1.0);
            let start_x = hud.x + (hud.width - total_w) / 2.0;
            let y = hud.y + (hud.height - btn_h) / 2.0;
            let mut x = start_x;
            for (i, w) in widths.iter().enumerate() {
                self.children[i].set_bounds(Rect::new(x, y, *w, btn_h));
                x += *w + STD_BTN_GAP;
            }
            self.children[ham].set_bounds(Rect::new(-9999.0, -9999.0, 0.0, 0.0));
        } else {
            // Compact: hamburger at the right edge of the HUD;
            // action buttons either off-screen (popup closed) or
            // stacked vertically above the hamburger (popup open).
            let probe = Size::new(hud.width, btn_h);
            let ham_w = self.children[ham].layout(probe).width;
            let ham_y = hud.y + (hud.height - btn_h) / 2.0;
            let ham_x = hud.x + hud.width - ham_w - STD_BTN_GAP;
            self.children[ham].set_bounds(Rect::new(ham_x, ham_y, ham_w, btn_h));
            if popup_open {
                let widths: Vec<f64> = (0..action_count)
                    .map(|i| self.children[i].layout(probe).width)
                    .collect();
                let col_w = widths.iter().cloned().fold(0.0_f64, f64::max).max(ham_w);
                let col_x = ham_x;
                // Stack upward from above the HUD strip.
                let bottom = hud.y + hud.height + STD_BTN_GAP;
                for (i, _w) in widths.iter().enumerate() {
                    let y = bottom + i as f64 * (btn_h + STD_BTN_GAP);
                    self.children[i].set_bounds(Rect::new(col_x, y, col_w, btn_h));
                }
            } else {
                for i in 0..action_count {
                    self.children[i].set_bounds(Rect::new(-9999.0, -9999.0, 0.0, 0.0));
                }
            }
        }
        let _ = chrome.mode;
    }

    /// Position + paint the Mom's-Solitaire shuffle counter.
    fn paint_moms_counter(&self, ctx: &mut dyn DrawCtx) {
        let model = self.model.borrow();
        if !matches!(model.kind, Some(crate::games::GameKind::MomsSolitaire)) {
            return;
        }
        let count = model.moms_shuffles;
        drop(model);
        let chrome = self.chrome();
        let hud = chrome.hud_rect;
        let label = format!("Shuffles: {count}");
        ctx.set_fill_color(TXT);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(16.0);
        let Some(m) = ctx.measure_text(&label) else {
            return;
        };
        let baseline = hud.y + m.centered_baseline_y(hud.height);
        ctx.fill_text(&label, hud.x + 18.0, baseline);
        let _ = chrome.mode;
    }
}

impl Widget for HudWidget {
    fn type_name(&self) -> &'static str {
        "HudWidget"
    }
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }
    fn layout(&mut self, available: Size) -> Size {
        self.sync_children();
        self.layout_buttons();
        available
    }

    fn is_visible(&self) -> bool {
        let s = self.model.borrow().screen;
        matches!(s, Screen::Game | Screen::Won)
    }

    /// Only claim pointer events that fall inside the HUD rect (or
    /// the open hamburger popup) for the current chrome mode.
    /// Without this override the widget's full-bounds default
    /// would swallow every click on the playfield and `GameWidget`
    /// (added earlier in the OverlayStack) would never receive a
    /// drag start.
    fn hit_test(&self, local_pos: Point) -> bool {
        let hud = self.chrome().hud_rect;
        let in_strip = local_pos.x >= hud.x
            && local_pos.x <= hud.x + hud.width
            && local_pos.y >= hud.y
            && local_pos.y <= hud.y + hud.height;
        if in_strip {
            return true;
        }
        // When the hamburger popup is open the action buttons
        // float above the HUD strip — claim their rect too so
        // the click reaches the popup button via the framework's
        // hit-test descent.
        if self.hamburger_open() {
            // Union of all action-button bounds (the popup
            // column).
            let action_count = self.children.len().saturating_sub(1);
            for i in 0..action_count {
                let b = self.children[i].bounds();
                if b.width > 0.0
                    && b.height > 0.0
                    && local_pos.x >= b.x
                    && local_pos.x <= b.x + b.width
                    && local_pos.y >= b.y
                    && local_pos.y <= b.y + b.height
                {
                    return true;
                }
            }
        }
        false
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        // The continuous chrome-strip background is painted once by the
        // felt layer (`AppRootWidget`) so the menu area and this HUD
        // area read as ONE bar. Here we only paint HUD-local content
        // (the Mom's shuffle counter); the Button children paint
        // themselves through the framework's child walk afterwards.
        self.paint_moms_counter(ctx);
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        // The Button children receive their own mouse events via
        // the framework's hit-test descent. We don't need any
        // local handling — let events fall through (Ignored) so
        // the playfield below sees them when the click misses the
        // HUD rect.
        EventResult::Ignored
    }

    /// Game-screen hotkeys. The framework only calls this if no
    /// focused widget consumed the key first, so dialogs (Confirm,
    /// Help) and menu popups still win.
    fn on_unconsumed_key(&mut self, key: &Key, modifiers: Modifiers) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        let Key::Char(c) = key else {
            return EventResult::Ignored;
        };
        let kind = self.model.borrow().kind;
        let lower = c.to_ascii_lowercase();
        match lower {
            'u' if !modifiers.ctrl && !modifiers.alt && !modifiers.meta => {
                let mut model = self.model.borrow_mut();
                if let Some(s) = model.session.as_mut() {
                    s.try_undo();
                }
                model.clear_spider_hint();
                agg_gui::animation::request_draw();
                EventResult::Consumed
            }
            'z' if modifiers.ctrl => {
                let mut model = self.model.borrow_mut();
                if let Some(s) = model.session.as_mut() {
                    s.try_undo();
                }
                model.clear_spider_hint();
                agg_gui::animation::request_draw();
                EventResult::Consumed
            }
            'h' if matches!(
                kind,
                Some(crate::games::GameKind::Spider) | Some(crate::games::GameKind::Klondike)
            ) && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.meta =>
            {
                self.model.borrow_mut().show_hint();
                agg_gui::animation::request_draw();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
