//! HUD widget — action buttons (Undo / New Deal / Hint / Shuffle /
//! Main Menu / Full Screen) plus the Mom's-Solitaire shuffle
//! counter. Visible whenever a game is active.
//!
//! Each action is an `agg_gui::widgets::Button` with a Font Awesome
//! glyph painted to the left of its label. The buttons live in
//! `self.children` and the framework's hit-test + paint walk does
//! the heavy lifting — the HUD itself just decides which subset
//! to show for the active variant and positions them in either:
//!
//! * `ChromeMode::Standard` — horizontal strip across the bottom
//!   of the viewport. Default for desktop and portrait phones.
//! * `ChromeMode::Sidebar` — vertical column on the LEFT side of
//!   the viewport with the menu bar still pinned to the top. Used
//!   on landscape-mobile so the playfield gets the full height.

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
use super::layout::{self, ChromeMode};

const HUD_BG: Color = Color::from_rgba8(0x09, 0x52, 0x2c, 0xe0);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BG_PRESSED: Color = Color::from_rgb8(0x18, 0x3d, 0x24);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);
const BTN_TEXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);
const TXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

/// Standard-mode button height + horizontal gap (horizontal strip).
const STD_BTN_H: f64 = 36.0;
const STD_BTN_GAP: f64 = 12.0;

/// Sidebar-mode button height and vertical gap.
const SIDE_BTN_H: f64 = 44.0;
const SIDE_BTN_GAP: f64 = 10.0;
const SIDE_PAD_X: f64 = 12.0;
const SIDE_PAD_TOP: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Fullscreen,
    Undo,
    NewDeal,
    Shuffle,
    Hint,
    Home,
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
            }
            Action::NewDeal => {
                if let Some(kind) = m.kind {
                    m.request_new_deal(kind);
                }
            }
            Action::Shuffle => {
                m.try_moms_shuffle();
            }
            Action::Hint => m.show_hint(),
            Action::Home => m.request_main_menu(),
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
            Some(crate::games::GameKind::Spider)
            | Some(crate::games::GameKind::Klondike) => vec![
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
    /// `Action` in its `on_click` closure.
    fn sync_children(&mut self) {
        let want = self.actions_for();
        if want == self.last_actions {
            return;
        }
        let _ = FA_BARS; // reserved for the hamburger follow-up
        self.children.clear();
        self.actions.clear();
        let font_size: f64 = 16.0;
        for action in &want {
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
        self.last_actions = want;
    }

    fn chrome(&self) -> layout::ChromeLayout {
        layout::compute(Size::new(self.bounds.width, self.bounds.height))
    }

    /// Position each button child for the active chrome mode.
    /// Standard: horizontal strip; Sidebar: vertical column.
    fn layout_buttons(&mut self) {
        let chrome = self.chrome();
        let hud = chrome.hud_rect;
        let n = self.children.len();
        if n == 0 {
            return;
        }
        match chrome.mode {
            ChromeMode::Standard => {
                // Ask each button for its natural width so wider
                // labels (Localised strings, longer game names)
                // grow the strip rather than truncating.
                let probe = Size::new(hud.width, STD_BTN_H);
                let widths: Vec<f64> = self
                    .children
                    .iter_mut()
                    .map(|c| c.layout(probe).width)
                    .collect();
                let total_w: f64 = widths.iter().sum::<f64>() + STD_BTN_GAP * (n as f64 - 1.0);
                let start_x = hud.x + (hud.width - total_w) / 2.0;
                let y = hud.y + (hud.height - STD_BTN_H) / 2.0;
                let mut x = start_x;
                for (i, w) in widths.iter().enumerate() {
                    self.children[i].set_bounds(Rect::new(x, y, *w, STD_BTN_H));
                    x += *w + STD_BTN_GAP;
                }
            }
            ChromeMode::Sidebar => {
                let btn_w = hud.width - SIDE_PAD_X * 2.0;
                let x = hud.x + SIDE_PAD_X;
                let top_of_first = hud.y + hud.height - SIDE_PAD_TOP;
                for (i, child) in self.children.iter_mut().enumerate() {
                    let y =
                        top_of_first - SIDE_BTN_H - i as f64 * (SIDE_BTN_H + SIDE_BTN_GAP);
                    child.set_bounds(Rect::new(x, y, btn_w, SIDE_BTN_H));
                    child.layout(Size::new(btn_w, SIDE_BTN_H));
                }
            }
        }
    }

    fn paint_strip(&self, ctx: &mut dyn DrawCtx, hud: Rect) {
        ctx.begin_path();
        ctx.rect(hud.x, hud.y, hud.width, hud.height);
        ctx.set_fill_color(HUD_BG);
        ctx.fill();
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
        match chrome.mode {
            ChromeMode::Standard => {
                let baseline = hud.y + m.centered_baseline_y(hud.height);
                ctx.fill_text(&label, hud.x + 18.0, baseline);
            }
            ChromeMode::Sidebar => {
                let baseline_y = hud.y + 12.0;
                let lx = hud.x + (hud.width - m.width) / 2.0;
                ctx.fill_text(&label, lx, baseline_y);
            }
        }
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

    /// Only claim pointer events that fall inside the HUD rect for
    /// the current chrome mode. Without this override the widget's
    /// full-bounds default would swallow every click on the
    /// playfield and `GameWidget` (added earlier in the
    /// OverlayStack) would never receive a drag start.
    fn hit_test(&self, local_pos: Point) -> bool {
        let hud = self.chrome().hud_rect;
        local_pos.x >= hud.x
            && local_pos.x <= hud.x + hud.width
            && local_pos.y >= hud.y
            && local_pos.y <= hud.y + hud.height
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let chrome = self.chrome();
        self.paint_strip(ctx, chrome.hud_rect);
        self.paint_moms_counter(ctx);
        // The Button children paint themselves through the
        // framework's child walk after this method returns.
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
