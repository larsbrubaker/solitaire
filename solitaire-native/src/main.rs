//! Native shell for the Solitaire game.
//!
//! # Platform-split policy (kept identical across `solitaire-native`, `solitaire-wasm`)
//!
//! This crate is a **platform shell only** — it wires up the OS window
//! (winit + wgpu surface), the event loop, input forwarding, and native
//! persistence. It contains **no game or UI content**: every game rule, widget
//! tree, menu, layout, and interface the user sees is shared via
//! `solitaire-core` (game logic + widget tree) and `demo-wgpu` (the wgpu
//! rendering library shared with agg-gui).
//!
//! - **Game / widget / layout code** → `solitaire-core`
//! - **GPU renderers (WGSL shaders, geometry, draw calls)** → `demo-wgpu`
//! - **Platform shell (OS window + event forwarding + persistence backend)** →
//!   here and `solitaire-wasm`

#![allow(deprecated)] // matches the agg-gui demo-native winit 0.30 idiom

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use agg_gui::{winit_adapter, App, Modifiers, Size};
use demo_wgpu::{begin_frame, WgpuGfxCtx};
use serde::{Deserialize, Serialize};
use solitaire_core::ui::build_solitaire_app;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position};
use winit::event::{ElementState, Event, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Fullscreen, Window, WindowAttributes};

const WINDOW_STATE_KEY: &str = "solitaire-native:window-state:v1";
const DEFAULT_WINDOW_W: u32 = 1024;
const DEFAULT_WINDOW_H: u32 = 768;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct WindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    fullscreen: bool,
}

impl WindowState {
    fn from_window(window: &Window) -> Self {
        let size = window.inner_size();
        let pos = window
            .outer_position()
            .unwrap_or_else(|_| PhysicalPosition::new(0, 0));
        Self {
            width: size.width.max(1),
            height: size.height.max(1),
            x: pos.x,
            y: pos.y,
            fullscreen: window.fullscreen().is_some(),
        }
    }

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

    fn save_from_window(window: &Window) {
        if window.fullscreen().is_some() {
            Self::save_fullscreen(true);
        } else {
            Self::from_window(window).save();
        }
    }

    fn save_fullscreen(fullscreen: bool) {
        let mut state = Self::load().unwrap_or(Self {
            width: DEFAULT_WINDOW_W,
            height: DEFAULT_WINDOW_H,
            x: 0,
            y: 0,
            fullscreen,
        });
        state.fullscreen = fullscreen;
        state.save();
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

fn register_native_fullscreen_toggle(window: Arc<Window>) {
    solitaire_core::platform::set_fullscreen_toggle(move || {
        let entering_fullscreen = window.fullscreen().is_none();
        let fullscreen = if entering_fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        };
        if entering_fullscreen {
            WindowState::save_from_window(&window);
        }
        window.set_fullscreen(fullscreen);
        WindowState::save_fullscreen(entering_fullscreen);
    });
}

fn save_window_size(window: &Window, size: PhysicalSize<u32>) {
    let mut state = WindowState::load().unwrap_or_else(|| WindowState::from_window(window));
    state.fullscreen = window.fullscreen().is_some();
    if !state.fullscreen {
        state.width = size.width.max(1);
        state.height = size.height.max(1);
    }
    state.save();
}

fn save_window_position(window: &Window, pos: PhysicalPosition<i32>) {
    let mut state = WindowState::load().unwrap_or_else(|| WindowState::from_window(window));
    state.fullscreen = window.fullscreen().is_some();
    if !state.fullscreen {
        state.x = pos.x;
        state.y = pos.y;
    }
    state.save();
}

struct Gpu {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    config: wgpu::SurfaceConfiguration,
}

impl Gpu {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(instance_desc);
        let surface = instance
            .create_surface(window.clone())
            .expect("create wgpu surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("request wgpu adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("solitaire-native-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        }))
        .expect("request wgpu device");

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface,
            surface_format,
            config,
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }
}

fn main() {
    let _ = dotenvy::dotenv();
    register_file_storage_io();

    let event_loop = EventLoop::new().expect("create event loop");
    let saved_window = WindowState::load();

    let mut window_attributes = WindowAttributes::default()
        .with_title("Solitaire")
        .with_inner_size(LogicalSize::new(
            saved_window.map_or(DEFAULT_WINDOW_W, |s| s.width),
            saved_window.map_or(DEFAULT_WINDOW_H, |s| s.height),
        ));
    if let Some(state) = saved_window {
        window_attributes = window_attributes
            .with_position(Position::Physical(PhysicalPosition::new(state.x, state.y)));
        if state.fullscreen {
            window_attributes =
                window_attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
    }

    let window = Arc::new(
        event_loop
            .create_window(window_attributes)
            .expect("create window"),
    );
    register_native_fullscreen_toggle(window.clone());
    agg_gui::set_device_scale(window.scale_factor());
    agg_gui::font_settings::set_lcd_enabled(agg_gui::device_scale() <= 1.25);

    let mut gpu = Gpu::new(window.clone());

    let (mut app, shared_model) = build_solitaire_app();
    let frame_history = shared_model.borrow().frame_history.clone();
    let mut wgpu_ctx = WgpuGfxCtx::new(
        Arc::clone(&gpu.device),
        Arc::clone(&gpu.queue),
        gpu.surface_format,
        gpu.config.width as f32,
        gpu.config.height as f32,
    );

    let mut win_w = window.inner_size().width.max(1);
    let mut win_h = window.inner_size().height.max(1);
    let mut cursor_x = 0.0_f64;
    let mut cursor_y = 0.0_f64;
    let mut current_mods = Modifiers::default();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                WindowState::save_from_window(&window);
                elwt.exit();
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } if size.width > 0 && size.height > 0 => {
                win_w = size.width;
                win_h = size.height;
                gpu.resize(win_w, win_h);
                save_window_size(&window, size);
                window.request_redraw();
            }

            Event::WindowEvent {
                event: WindowEvent::Moved(pos),
                ..
            } => {
                save_window_position(&window, pos);
                window.request_redraw();
            }

            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { scale_factor, .. },
                ..
            } => {
                agg_gui::set_device_scale(scale_factor);
                window.request_redraw();
            }

            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                cursor_x = position.x;
                cursor_y = position.y;
                app.on_mouse_move(cursor_x, cursor_y);
                winit_adapter::apply_cursor(&window, agg_gui::current_cursor_icon());
            }

            Event::WindowEvent {
                event: WindowEvent::CursorLeft { .. },
                ..
            } => {
                app.on_mouse_leave();
            }

            Event::WindowEvent {
                event: WindowEvent::ModifiersChanged(mods_state),
                ..
            } => {
                current_mods = winit_adapter::modifiers(mods_state.state());
            }

            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                let btn = winit_adapter::mouse_button(button);
                match state {
                    ElementState::Pressed => {
                        app.on_mouse_down(cursor_x, cursor_y, btn, current_mods);
                    }
                    ElementState::Released => {
                        app.on_mouse_up(cursor_x, cursor_y, btn, current_mods);
                    }
                }
            }

            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(dx, dy),
                        ..
                    },
                ..
            } => {
                app.on_mouse_wheel_xy_mods(cursor_x, cursor_y, dx as f64, dy as f64, current_mods);
            }

            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event: key_event, ..
                    },
                ..
            } => {
                let Some(key) = winit_adapter::key_event(&key_event, current_mods) else {
                    return;
                };
                match key_event.state {
                    ElementState::Pressed => {
                        app.on_key_down(key, current_mods);
                    }
                    ElementState::Released => {
                        app.on_key_up(key, current_mods);
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let render_start = Instant::now();
                paint_frame(&gpu, &mut wgpu_ctx, &mut app, win_w, win_h);
                let render_ms = render_start.elapsed().as_secs_f64() * 1000.0;
                // Feed the shared model's `FrameHistory` so the in-app
                // Performance window (Debug menu → "Performance Window")
                // can render the rolling mean + sparkline.  No console
                // print: the on-screen widget is the supported way to
                // inspect frame timing now.
                frame_history.borrow_mut().push(render_ms as f32);
            }

            Event::AboutToWait => {
                window.request_redraw();
            }

            _ => {}
        })
        .expect("event loop");
}

fn paint_frame(gpu: &Gpu, ctx: &mut WgpuGfxCtx, app: &mut App, win_w: u32, win_h: u32) {
    if win_w == 0 || win_h == 0 {
        return;
    }
    let Some(frame) = acquire_frame(gpu) else {
        return;
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    ctx.set_surface_texture(frame.texture.clone());
    ctx.reset(win_w as f32, win_h as f32);
    ctx.set_lcd_mode(agg_gui::font_settings::lcd_enabled());
    begin_frame(ctx, view);
    app.layout(Size::new(win_w as f64, win_h as f64));
    app.paint(ctx);
    ctx.end_frame();
    frame.present();
}

fn acquire_frame(gpu: &Gpu) -> Option<wgpu::SurfaceTexture> {
    match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            Some(f)
        }
        _ => None,
    }
}
