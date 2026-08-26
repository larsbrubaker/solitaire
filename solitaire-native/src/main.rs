//! Native shell for the Solitaire game.
//!
//! # Platform-split policy (kept identical across `solitaire-native`, `solitaire-wasm`)
//!
//! This crate is a **platform shim only** — the OS window, event loop, input
//! forwarding and wgpu present are `agg-gui-shell`'s. It contains **no game or
//! UI content**: every game rule, widget tree, menu, layout, and interface the
//! user sees is shared via `solitaire-core` (game logic + widget tree) and
//! `agg-gui-wgpu` (the wgpu rendering library shared with agg-gui).
//!
//! - **Game / widget / layout code** → `solitaire-core`
//! - **GPU renderers (WGSL shaders, geometry, draw calls)** → `agg-gui-wgpu`
//! - **Window, event loop, input, present, bounds persistence** →
//!   `agg-gui-shell`
//! - **What is left here** (the glue the shell's hooks can't express):
//!   `.env` + file-backed settings registration, window *position* and
//!   *fullscreen-flag* persistence (the shell's [`WindowBoundsStore`] only
//!   carries size + maximized), the stale-monitor position recovery, the
//!   fullscreen-toggle bridge into `solitaire_core::platform`, and the
//!   Performance-window frame-history feed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agg_gui::App;
use agg_gui_shell::winit::dpi::PhysicalPosition;
use agg_gui_shell::winit::window::Window;
use agg_gui_shell::{
    run, Frame, SavedBounds, ShellConfig, ShellControl, ShellError, ShellHost, WindowBoundsStore,
};
use serde::{Deserialize, Serialize};
use solitaire_core::ui::build_solitaire_app;

const WINDOW_STATE_KEY: &str = "solitaire-native:window-state:v1";
const DEFAULT_WINDOW_W: u32 = 1024;
const DEFAULT_WINDOW_H: u32 = 768;

/// The persisted window placement, stored as one JSON blob under
/// [`WINDOW_STATE_KEY`] in the settings file. Size + `maximized` flow through
/// the shell's [`WindowBoundsStore`]; position and the fullscreen flag are
/// saved by [`SolitaireHost::save_placement`] because the shell does not
/// persist either.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct WindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    fullscreen: bool,
    /// Added in the agg-gui-shell port; `default` keeps old state files
    /// (which had no maximized tracking) loading.
    #[serde(default)]
    maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_W,
            height: DEFAULT_WINDOW_H,
            x: 0,
            y: 0,
            fullscreen: false,
            maximized: false,
        }
    }
}

impl WindowState {
    fn is_reasonable(self) -> bool {
        (320..=7680).contains(&self.width)
            && (240..=4320).contains(&self.height)
            && self.x.abs() <= 100_000
            && self.y.abs() <= 100_000
    }

    fn load() -> Option<Self> {
        let raw = solitaire_core::platform::storage_load(WINDOW_STATE_KEY)?;
        serde_json::from_str::<Self>(&raw)
            .ok()
            .filter(|s| s.is_reasonable())
    }

    fn save(self) {
        if let Ok(raw) = serde_json::to_string(&self) {
            solitaire_core::platform::storage_save(WINDOW_STATE_KEY, &raw);
        }
    }
}

/// Read-modify-write the persisted [`WindowState`] so the two writers (the
/// shell's bounds store for size/maximized, the host's placement saver for
/// position/fullscreen) never clobber each other's fields. The event loop is
/// single-threaded, so there is no write race.
fn update_window_state(f: impl FnOnce(&mut WindowState)) {
    let mut state = WindowState::load().unwrap_or_default();
    f(&mut state);
    state.save();
}

/// The shell's view of the persisted bounds: size + maximized, backed by the
/// same [`WindowState`] blob as the position/fullscreen glue.
struct SettingsBounds;

impl WindowBoundsStore for SettingsBounds {
    fn load(&self) -> Option<SavedBounds> {
        WindowState::load().map(|s| SavedBounds {
            width: s.width,
            height: s.height,
            maximized: s.maximized,
        })
    }

    fn save(&self, bounds: SavedBounds) {
        update_window_state(|s| {
            s.width = bounds.width;
            s.height = bounds.height;
            s.maximized = bounds.maximized;
        });
    }
}

fn storage_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("Solitaire");
    path.push("settings.json");
    Some(path)
}

fn read_storage_file(path: &PathBuf) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_storage_file(path: &PathBuf, store: &HashMap<String, String>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(store) {
        let _ = std::fs::write(path, s);
    }
}

fn register_file_storage_io() {
    let Some(path) = storage_path() else {
        return;
    };
    let load_path = path.clone();
    solitaire_core::platform::set_storage_io(
        move |key| read_storage_file(&load_path).get(key).cloned(),
        move |key, value| {
            let mut store = read_storage_file(&path);
            store.insert(key.to_string(), value.to_string());
            write_storage_file(&path, &store);
        },
    );
}

/// Returns true if at least a 100x100 patch of the saved window rect
/// overlaps a currently-connected monitor. Used to drop a stale saved
/// position from a monitor that's no longer attached (laptop undocked,
/// external display unplugged) so the window doesn't open off-screen
/// and become unreachable.
fn position_visible_on_any_monitor(window: &Window, state: &WindowState) -> bool {
    let win_left = state.x;
    let win_top = state.y;
    let win_right = state.x.saturating_add(state.width as i32);
    let win_bottom = state.y.saturating_add(state.height as i32);
    for m in window.available_monitors() {
        let mp = m.position();
        let ms = m.size();
        let m_left = mp.x;
        let m_top = mp.y;
        let m_right = mp.x.saturating_add(ms.width as i32);
        let m_bottom = mp.y.saturating_add(ms.height as i32);
        let overlap_w = win_right.min(m_right) - win_left.max(m_left);
        let overlap_h = win_bottom.min(m_bottom) - win_top.max(m_top);
        if overlap_w >= 100 && overlap_h >= 100 {
            return true;
        }
    }
    false
}

/// Restore the saved window position, or center on the primary monitor when
/// the saved rect no longer overlaps any connected display. Runs from the
/// shell's build closure while the window is still hidden, so the move is
/// invisible (winit 0.30 exposes monitor enumeration only on a live window,
/// which is why this can't happen before window creation).
fn restore_window_position(window: &Window, state: &WindowState) {
    if position_visible_on_any_monitor(window, state) {
        window.set_outer_position(PhysicalPosition::new(state.x, state.y));
    } else if let Some(primary) = window
        .primary_monitor()
        .or_else(|| window.current_monitor())
    {
        let mp = primary.position();
        let ms = primary.size();
        let cx = mp.x + ((ms.width as i32 - state.width as i32) / 2).max(0);
        let cy = mp.y + ((ms.height as i32 - state.height as i32) / 2).max(0);
        window.set_outer_position(PhysicalPosition::new(cx, cy));
    }
}

/// Everything `agg-gui-shell` does not own: the Performance window's
/// frame-history feed (plus the one-frame flush that makes the just-recorded
/// sample visible), and window position / fullscreen-flag persistence.
struct SolitaireHost {
    window: Arc<Window>,
    frame_history: agg_gui::SharedFrameHistory,
    perf_window_visible: std::rc::Rc<std::cell::Cell<bool>>,
    /// True while the frame the loop is painting was requested solely to
    /// flush a Performance-window sample; such a frame must not schedule
    /// another flush or the measurement display would repaint forever.
    perf_flush_frame: bool,
    /// Last placement written, so idle ticks only touch the settings file on
    /// an actual change. `(position, fullscreen)`.
    last_placement: Option<(Option<(i32, i32)>, bool)>,
}

impl SolitaireHost {
    /// Persist position + fullscreen flag on change. Size and maximized are
    /// the shell's job (via [`SettingsBounds`]); position is skipped while
    /// maximized or fullscreen so a monitor-origin rect is never recorded as
    /// the windowed position.
    fn save_placement(&mut self) {
        let fullscreen = self.window.fullscreen().is_some();
        let pos = if fullscreen || self.window.is_maximized() {
            None
        } else {
            self.window.outer_position().ok().map(|p| (p.x, p.y))
        };
        let placement = (pos, fullscreen);
        if self.last_placement == Some(placement) {
            return;
        }
        self.last_placement = Some(placement);
        update_window_state(|s| {
            s.fullscreen = fullscreen;
            if let Some((x, y)) = pos {
                s.x = x;
                s.y = y;
            }
        });
    }
}

impl ShellHost for SolitaireHost {
    fn on_frame(&mut self, _app: &mut App, frame: &Frame) {
        // `frame.duration` is the previous painted frame's wall time (paint +
        // present) — the number the in-app Performance window plots. The first
        // frame has no predecessor, so nothing to record yet.
        if frame.index > 1 {
            self.frame_history
                .borrow_mut()
                .push(frame.duration.as_secs_f32() * 1000.0);
        }
    }

    fn on_idle(&mut self, _app: &mut App, control: &mut ShellControl<'_>) {
        // A normal draw records its sample after the PerformanceView has
        // already painted, so schedule ONE follow-up frame while the
        // Performance window is open to make that just-recorded sample
        // visible. The flush frame itself does not schedule another —
        // otherwise the measurement display would keep the app rendering
        // forever.
        if control.painted() {
            let was_flush = self.perf_flush_frame;
            self.perf_flush_frame = false;
            if self.perf_window_visible.get() && !was_flush {
                self.perf_flush_frame = true;
                agg_gui::animation::request_draw();
            }
        }

        // Same gate the shell applies to its bounds store: never write the
        // settings file mid-drag.
        if control.pointer_idle() {
            self.save_placement();
        }
    }

    fn on_exit(&mut self, _app: &mut App) {
        self.save_placement();
    }
}

fn main() -> Result<(), ShellError> {
    let _ = dotenvy::dotenv();
    register_file_storage_io();

    let saved_window = WindowState::load();
    let (icon_w, icon_h, icon_rgba) = solitaire_core::branding::app_icon_rgba();

    let config = ShellConfig::new("Solitaire")
        .with_logical_size(f64::from(DEFAULT_WINDOW_W), f64::from(DEFAULT_WINDOW_H))
        .with_icon_rgba(icon_rgba, icon_w, icon_h)
        .with_device_label("solitaire-native-wgpu")
        .with_fullscreen(saved_window.is_some_and(|s| s.fullscreen))
        .with_bounds_store(SettingsBounds);

    run(config, move |init| {
        let window = Arc::clone(init.window());

        // The window exists but is still hidden: restore the saved position
        // (with stale-monitor recovery) before the first frame is shown.
        if let Some(state) = saved_window.filter(|s| !s.fullscreen) {
            if !window.is_maximized() {
                restore_window_position(&window, &state);
            }
        }

        // The Options-menu fullscreen item calls
        // `solitaire_core::platform::request_toggle_fullscreen`; bridge it to
        // the shell's toggle path (`agg_gui::fullscreen`), which owns the
        // winit fullscreen transition and the active-state bookkeeping.
        solitaire_core::platform::set_fullscreen_toggle(agg_gui::fullscreen::request_toggle);

        // LCD subpixel text below ~1.25x scale, grayscale AA above — matches
        // the wasm shell. The device scale is already known here.
        agg_gui::font_settings::set_lcd_enabled(agg_gui::device_scale() <= 1.25);

        let (app, shared_model) = build_solitaire_app();
        let frame_history = shared_model.borrow().frame_history.clone();
        let perf_window_visible = shared_model.borrow().show_performance_window.clone();

        Ok((
            app,
            SolitaireHost {
                window,
                frame_history,
                perf_window_visible,
                perf_flush_frame: false,
                last_placement: None,
            },
        ))
    })
}
