//! HUD widget — Undo / New Deal / (Shuffle) / Main Menu buttons plus the
//! Mom's-Solitaire shuffle counter. Visible whenever a game is active.
//!
//! Paints in viewport coordinates rather than the virtual playfield
//! coordinate space so it can switch between two chrome layouts driven
//! by [`layout::compute`]:
//!
//! * `ChromeMode::Standard` — horizontal strip across the bottom of the
//!   viewport. Default for desktop and portrait phones.
//! * `ChromeMode::Sidebar` — vertical column on the LEFT side of the
//!   viewport with the menu bar still pinned to the top. Used on
//!   landscape-mobile so the playfield gets the full window height.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key, Modifiers, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use super::app_model::{Screen, SharedModel};
use super::layout::{self, ChromeMode};

const HUD_BG: Color = Color::from_rgba8(0x09, 0x52, 0x2c, 0xe0);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);
const TXT: Color = Color::from_rgb8(0xff, 0xff, 0xff);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    Fullscreen,
    Undo,
    NewDeal,
    Shuffle,
    Hint,
    Home,
}

/// Standard-mode button height/width and gap (horizontal strip).
const STD_BTN_W: f64 = 120.0;
const STD_BTN_H: f64 = 36.0;
const STD_BTN_GAP: f64 = 12.0;

/// Sidebar-mode button height and vertical gap. Width derives from the
/// sidebar column width minus padding so buttons fit a 44-ish px tap
/// target on a phone.
const SIDE_BTN_H: f64 = 44.0;
const SIDE_BTN_GAP: f64 = 10.0;
const SIDE_PAD_X: f64 = 12.0;
const SIDE_PAD_TOP: f64 = 12.0;

pub struct HudWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    hover: Option<Btn>,
}

impl HudWidget {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            hover: None,
        }
    }

    /// Buttons to render. Mom's Solitaire gets an extra `Shuffle`
    /// between New Deal and Main Menu; Spider gets an extra `Hint`.
    /// Everything else gets the standard three. The menu-bar / sidebar-
    /// menu actions (Restart, Draw 1/3, Rules, About) live in the menu
    /// widget — not duplicated here.
    fn btns(&self) -> Vec<Btn> {
        match self.model.borrow().kind {
            Some(crate::games::GameKind::MomsSolitaire) => vec![
                Btn::Fullscreen,
                Btn::Undo,
                Btn::NewDeal,
                Btn::Shuffle,
                Btn::Home,
            ],
            Some(crate::games::GameKind::Spider) => vec![
                Btn::Fullscreen,
                Btn::Undo,
                Btn::NewDeal,
                Btn::Hint,
                Btn::Home,
            ],
            _ => vec![Btn::Fullscreen, Btn::Undo, Btn::NewDeal, Btn::Home],
        }
    }

    fn btn_label(&self, b: Btn) -> &'static str {
        match b {
            Btn::Fullscreen => "Full Screen",
            Btn::Undo => "Undo",
            Btn::NewDeal => "New Deal",
            Btn::Shuffle => "Shuffle",
            Btn::Hint => "Hint",
            Btn::Home => "Main Menu",
        }
    }

    /// Resolve the current chrome layout for `self.bounds`.
    fn chrome(&self) -> layout::ChromeLayout {
        layout::compute(Size::new(self.bounds.width, self.bounds.height))
    }

    /// Pixel rect for the `idx`-th button in viewport coords. `n` is the
    /// total button count for the active variant.
    fn btn_rect_for(&self, idx: usize, n: usize) -> (f64, f64, f64, f64) {
        let chrome = self.chrome();
        let hud = chrome.hud_rect;
        match chrome.mode {
            ChromeMode::Standard => {
                let total_w = n as f64 * STD_BTN_W + (n as f64 - 1.0) * STD_BTN_GAP;
                let start_x = hud.x + (hud.width - total_w) / 2.0;
                let x = start_x + idx as f64 * (STD_BTN_W + STD_BTN_GAP);
                let y = hud.y + (hud.height - STD_BTN_H) / 2.0;
                (x, y, STD_BTN_W, STD_BTN_H)
            }
            ChromeMode::Sidebar => {
                let btn_w = hud.width - SIDE_PAD_X * 2.0;
                let x = hud.x + SIDE_PAD_X;
                // Y-up: idx 0 is the TOP-most button in the column. The
                // top of the column is at hud.y + hud.height.
                let top_of_first = hud.y + hud.height - SIDE_PAD_TOP;
                let y = top_of_first - SIDE_BTN_H - idx as f64 * (SIDE_BTN_H + SIDE_BTN_GAP);
                (x, y, btn_w, SIDE_BTN_H)
            }
        }
    }

    fn hit_btn_at(&self, px: f64, py: f64) -> Option<Btn> {
        let btns = self.btns();
        let n = btns.len();
        for (i, b) in btns.iter().enumerate() {
            let (x, y, w, h) = self.btn_rect_for(i, n);
            if px >= x && px <= x + w && py >= y && py <= y + h {
                return Some(*b);
            }
        }
        None
    }

    fn click_at(&mut self, px: f64, py: f64) -> bool {
        let Some(b) = self.hit_btn_at(px, py) else {
            return false;
        };
        let mut model = self.model.borrow_mut();
        match b {
            Btn::Fullscreen => {
                crate::platform::request_toggle_fullscreen();
            }
            Btn::Undo => {
                if let Some(s) = model.session.as_mut() {
                    s.try_undo();
                }
                model.clear_spider_hint();
            }
            Btn::NewDeal => {
                if let Some(kind) = model.kind {
                    model.request_new_deal(kind);
                }
            }
            Btn::Shuffle => {
                model.try_moms_shuffle();
            }
            Btn::Hint => {
                model.show_spider_hint();
            }
            Btn::Home => {
                model.request_main_menu();
            }
        }
        agg_gui::animation::request_draw();
        true
    }

    fn paint_strip(&self, ctx: &mut dyn DrawCtx, hud: Rect) {
        ctx.begin_path();
        ctx.rect(hud.x, hud.y, hud.width, hud.height);
        ctx.set_fill_color(HUD_BG);
        ctx.fill();
    }

    fn paint_btn(&self, ctx: &mut dyn DrawCtx, idx: usize, b: Btn, n: usize) {
        let (x, y, w, h) = self.btn_rect_for(idx, n);
        let bg = if self.hover == Some(b) {
            BTN_BG_HOVER
        } else {
            BTN_BG
        };
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 8.0);
        ctx.set_fill_color(bg);
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 8.0);
        ctx.set_stroke_color(BTN_BORDER);
        ctx.set_line_width(1.5);
        ctx.stroke();

        ctx.set_fill_color(TXT);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(18.0);
        let label = self.btn_label(b);
        let Some(m) = ctx.measure_text(label) else {
            return;
        };
        let lx = x + (w - m.width) / 2.0;
        let ly = y + m.centered_baseline_y(h);
        ctx.fill_text(label, lx, ly);
    }

    /// Position + paint the Mom's-Solitaire shuffle counter. Tucked
    /// against the leading edge of the HUD in Standard mode and below
    /// the button stack in Sidebar mode so it never collides with the
    /// buttons.
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
                // Place the counter near the BOTTOM of the sidebar
                // column (small numerical y in Y-up) so it's out of the
                // way of the button stack at the top.
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
        available
    }

    fn is_visible(&self) -> bool {
        let s = self.model.borrow().screen;
        matches!(s, Screen::Game | Screen::Won)
    }

    /// Only claim pointer events that fall inside the HUD rect for the
    /// current chrome mode. Without this override the widget's full-
    /// bounds default would swallow every click on the playfield and
    /// `GameWidget` (added earlier in the OverlayStack) would never
    /// receive a drag start.
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
        let btns = self.btns();
        let n = btns.len();
        for (i, b) in btns.iter().enumerate() {
            self.paint_btn(ctx, i, *b, n);
        }
        self.paint_moms_counter(ctx);
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        if !self.is_visible() {
            return EventResult::Ignored;
        }
        let hud = self.chrome().hud_rect;
        let inside = |p: Point| -> bool {
            p.x >= hud.x && p.x <= hud.x + hud.width && p.y >= hud.y && p.y <= hud.y + hud.height
        };
        match event {
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if inside(*pos) && self.click_at(pos.x, pos.y) {
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            Event::MouseMove { pos } => {
                let new_hover = if inside(*pos) {
                    self.hit_btn_at(pos.x, pos.y)
                } else {
                    None
                };
                if new_hover != self.hover {
                    self.hover = new_hover;
                    agg_gui::animation::request_draw();
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
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
            'h' if matches!(kind, Some(crate::games::GameKind::Spider))
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.meta =>
            {
                self.model.borrow_mut().show_spider_hint();
                agg_gui::animation::request_draw();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
