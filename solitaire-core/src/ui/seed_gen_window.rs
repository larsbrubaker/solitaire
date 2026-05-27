//! Content view for the seed-generator window — two columns
//! (Spider | Klondike) showing live counters plus a tail-scrolling
//! log of recent solver outcomes. Polls
//! `crate::games::seed_generator`'s `Arc<Mutex<SeedGenStatus>>`
//! singletons on every paint; the worker thread writes them.
//!
//! Designed to live inside an `agg_gui::widgets::Window` so it
//! gets a draggable / closable chrome for free.

use std::sync::{Arc, Mutex};

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, MouseButton};
use agg_gui::geometry::{Point, Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::games::seed_generator::{
    klondike_status, seed_generation_running, spider_status, start_seed_generation,
    stop_seed_generation, SeedGenStatus, TARGET_SEED_COUNT,
};

const BG: Color = Color::from_rgb8(0x10, 0x1f, 0x16);
const HEADER: Color = Color::from_rgb8(0xff, 0xd7, 0x00);
const TEXT: Color = Color::from_rgb8(0xe6, 0xe6, 0xe6);
const TEXT_DIM: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0xb0);
const PROG_TRACK: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x30);
const PROG_FILL: Color = Color::from_rgb8(0x4c, 0xc4, 0x70);
const BTN_BG: Color = Color::from_rgb8(0x1f, 0x4d, 0x2e);
const BTN_BG_HOVER: Color = Color::from_rgb8(0x29, 0x68, 0x3e);
const BTN_BORDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x80);
const DIVIDER: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x22);

const PAD: f64 = 12.0;
const BTN_H: f64 = 32.0;
const BTN_W: f64 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Btn {
    Start,
    Stop,
}

pub struct SeedGenView {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    font: Arc<Font>,
    spider: Arc<Mutex<SeedGenStatus>>,
    klondike: Arc<Mutex<SeedGenStatus>>,
    hover: Option<Btn>,
}

impl SeedGenView {
    pub fn new(font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            font,
            spider: spider_status(),
            klondike: klondike_status(),
            hover: None,
        }
    }

    fn button_rect(&self, btn: Btn) -> (f64, f64, f64, f64) {
        // Buttons sit at the BOTTOM of the panel in screen Y-down,
        // which is `bounds.y + PAD` in Y-up.
        let y = self.bounds.y + PAD;
        let total = BTN_W * 2.0 + 16.0;
        let start_x = self.bounds.x + (self.bounds.width - total) / 2.0;
        let x = match btn {
            Btn::Start => start_x,
            Btn::Stop => start_x + BTN_W + 16.0,
        };
        (x, y, BTN_W, BTN_H)
    }

    fn hit_button(&self, p: Point) -> Option<Btn> {
        for b in [Btn::Start, Btn::Stop] {
            let (x, y, w, h) = self.button_rect(b);
            if p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h {
                return Some(b);
            }
        }
        None
    }

    fn paint_button(&self, ctx: &mut dyn DrawCtx, btn: Btn, label: &str, enabled: bool) {
        let (x, y, w, h) = self.button_rect(btn);
        let bg = if !enabled {
            Color::from_rgba8(0x1f, 0x4d, 0x2e, 0x80)
        } else if self.hover == Some(btn) {
            BTN_BG_HOVER
        } else {
            BTN_BG
        };
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 6.0);
        ctx.set_fill_color(bg);
        ctx.fill();
        ctx.begin_path();
        ctx.rounded_rect(x, y, w, h, 6.0);
        ctx.set_stroke_color(BTN_BORDER);
        ctx.set_line_width(1.0);
        ctx.stroke();
        ctx.set_fill_color(if enabled { TEXT } else { TEXT_DIM });
        ctx.set_font(self.font.clone());
        ctx.set_font_size(15.0);
        if let Some(m) = ctx.measure_text(label) {
            ctx.fill_text(label, x + (w - m.width) / 2.0, y + m.centered_baseline_y(h));
        }
    }

    fn paint_column(
        &self,
        ctx: &mut dyn DrawCtx,
        column_rect: Rect,
        title: &str,
        status: &SeedGenStatus,
    ) {
        ctx.set_font(self.font.clone());

        // Header.
        let header_y_baseline = column_rect.y + column_rect.height - 22.0;
        ctx.set_fill_color(HEADER);
        ctx.set_font_size(18.0);
        ctx.fill_text(title, column_rect.x, header_y_baseline);

        // Counter line.
        let counter = format!(
            "won {} / lost {} / timeout {}  (target {})",
            status.won, status.lost, status.timed_out, TARGET_SEED_COUNT
        );
        ctx.set_fill_color(TEXT);
        ctx.set_font_size(13.0);
        ctx.fill_text(&counter, column_rect.x, header_y_baseline - 22.0);

        // Status line: running / stopped / finished.
        let state_text = if status.finished {
            "complete".to_string()
        } else if status.running {
            format!("running (currently at seed {})", status.current_seed)
        } else {
            "idle".to_string()
        };
        ctx.set_fill_color(TEXT_DIM);
        ctx.fill_text(&state_text, column_rect.x, header_y_baseline - 40.0);

        // Progress bar.
        let bar_y_top = header_y_baseline - 58.0;
        let bar_y = bar_y_top - 10.0; // Y-up: rect's bottom is bar_y.
        let bar_w = column_rect.width;
        ctx.begin_path();
        ctx.rounded_rect(column_rect.x, bar_y, bar_w, 10.0, 4.0);
        ctx.set_fill_color(PROG_TRACK);
        ctx.fill();
        let frac = (status.won as f64 / TARGET_SEED_COUNT as f64).min(1.0);
        if frac > 0.0 {
            ctx.begin_path();
            ctx.rounded_rect(column_rect.x, bar_y, bar_w * frac, 10.0, 4.0);
            ctx.set_fill_color(PROG_FILL);
            ctx.fill();
        }

        // Log: paint tail of buffer, newest at top, oldest fading
        // down. Roughly fills space between bar bottom and column
        // bottom edge.
        let log_top = bar_y - 8.0;
        let log_bottom = column_rect.y;
        let line_h = 14.0;
        let lines_visible = ((log_top - log_bottom) / line_h).floor() as usize;
        ctx.set_font_size(11.5);
        ctx.set_fill_color(TEXT_DIM);
        let log: Vec<&String> = status.log.iter().collect();
        let tail = log.iter().rev().take(lines_visible);
        let mut y = log_top - 12.0;
        for line in tail {
            if y < log_bottom {
                break;
            }
            ctx.fill_text(line.as_str(), column_rect.x, y);
            y -= line_h;
        }
    }
}

impl Widget for SeedGenView {
    fn type_name(&self) -> &'static str {
        "SeedGenView"
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
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        // Background fill behind the view (the agg-gui Window paints
        // its own chrome but the content rect is transparent).
        ctx.begin_path();
        ctx.rect(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.set_fill_color(BG);
        ctx.fill();

        // Snapshot both status blocks under brief locks so we don't
        // hold a mutex while painting.
        let spider_snap = self.spider.lock().map(|s| s.clone()).unwrap_or_default();
        let klondike_snap = self.klondike.lock().map(|s| s.clone()).unwrap_or_default();

        let inner_x = self.bounds.x + PAD;
        let inner_w = self.bounds.width - PAD * 2.0;
        let buttons_top = self.bounds.y + PAD + BTN_H + PAD; // Y-up
        let inner_top = self.bounds.y + self.bounds.height - PAD;
        let column_height = (inner_top - buttons_top).max(60.0);
        let col_w = (inner_w - PAD) / 2.0;
        let spider_rect = Rect::new(inner_x, buttons_top, col_w, column_height);
        let klondike_rect = Rect::new(inner_x + col_w + PAD, buttons_top, col_w, column_height);

        self.paint_column(ctx, spider_rect, "Spider (4-suit)", &spider_snap);
        self.paint_column(ctx, klondike_rect, "Klondike", &klondike_snap);

        // Vertical divider between columns.
        ctx.begin_path();
        let div_x = inner_x + col_w + PAD / 2.0;
        ctx.rect(div_x - 0.5, buttons_top, 1.0, column_height);
        ctx.set_fill_color(DIVIDER);
        ctx.fill();

        // Buttons.
        let running = seed_generation_running();
        self.paint_button(ctx, Btn::Start, "Start", !running);
        self.paint_button(ctx, Btn::Stop, "Stop", running);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => match self.hit_button(*pos) {
                Some(Btn::Start) => {
                    start_seed_generation();
                    agg_gui::animation::request_draw();
                    EventResult::Consumed
                }
                Some(Btn::Stop) => {
                    stop_seed_generation();
                    agg_gui::animation::request_draw();
                    EventResult::Consumed
                }
                None => EventResult::Ignored,
            },
            Event::MouseMove { pos } => {
                let h = self.hit_button(*pos);
                if h != self.hover {
                    self.hover = h;
                    agg_gui::animation::request_draw();
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
    fn needs_draw(&self) -> bool {
        seed_generation_running()
    }
}
