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

use std::sync::Arc;
use std::time::{Duration, Instant};

use agg_gui::{winit_adapter, App, Modifiers, Size};
use demo_wgpu::{begin_frame, WgpuGfxCtx};
use solitaire_core::ui::build_solitaire_app;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowAttributes};

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

    let event_loop = EventLoop::new().expect("create event loop");

    let window_attributes = WindowAttributes::default()
        .with_title("Solitaire")
        .with_inner_size(LogicalSize::new(1024, 768));

    let window = Arc::new(
        event_loop
            .create_window(window_attributes)
            .expect("create window"),
    );
    agg_gui::set_device_scale(window.scale_factor());
    agg_gui::font_settings::set_lcd_enabled(agg_gui::device_scale() <= 1.25);

    let mut gpu = Gpu::new(window.clone());

    let mut app = build_solitaire_app();
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

    // Roll-up of paint-only render times (i.e. the `paint_frame` call
    // duration, NOT the wall-clock interval between frames). Reported
    // every `RENDER_REPORT_INTERVAL`; aggregated to keep stdout quiet.
    const RENDER_REPORT_INTERVAL: Duration = Duration::from_secs(3);
    let mut render_ms_sum: f64 = 0.0;
    let mut render_ms_max: f64 = 0.0;
    let mut render_frames: u32 = 0;
    let mut render_window_start = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),

            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } if size.width > 0 && size.height > 0 => {
                win_w = size.width;
                win_h = size.height;
                gpu.resize(win_w, win_h);
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
                render_ms_sum += render_ms;
                if render_ms > render_ms_max {
                    render_ms_max = render_ms;
                }
                render_frames += 1;
                if render_window_start.elapsed() >= RENDER_REPORT_INTERVAL && render_frames > 0 {
                    let avg = render_ms_sum / render_frames as f64;
                    eprintln!(
                        "solitaire: render avg {avg:.2} ms (peak {render_ms_max:.2} ms) over last {render_frames} frames"
                    );
                    render_ms_sum = 0.0;
                    render_ms_max = 0.0;
                    render_frames = 0;
                    render_window_start = Instant::now();
                }
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
