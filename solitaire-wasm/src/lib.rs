#![cfg(target_arch = "wasm32")]
//! WASM shell for Solitaire — browser canvas + wgpu/WebGL rendering.
//!
//! # Platform-split policy (kept identical across `solitaire-native`, `solitaire-wasm`)
//!
//! This crate is a **platform shell only** — canvas, browser events,
//! `localStorage` persistence, and wasm-bindgen exports. It contains **no game
//! or UI content**: every game rule, widget tree, menu, layout, and interface
//! the user sees is shared via `solitaire-core` (game logic + widget tree) and
//! `demo-wgpu` (the wgpu rendering library shared with agg-gui).

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use agg_gui::{App, Modifiers, MouseButton};
use demo_wgpu::{begin_frame, WgpuGfxCtx};
use solitaire_core::ui::build_solitaire_app;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static WGPU_INIT: RefCell<Option<WgpuInit>> = const { RefCell::new(None) };
    static WGPU_CTX: RefCell<Option<WgpuGfxCtx>> = const { RefCell::new(None) };
    static NEEDS_DRAW: Cell<bool> = const { Cell::new(true) };
}

struct WgpuInit {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    config: wgpu::SurfaceConfiguration,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    // Register the localStorage-backed K/V store BEFORE building the
    // app so `AppModel::new()`'s call to `UserSettings::load()` sees a
    // live backend on its first read. Failures here (e.g. private
    // mode, storage disabled) silently fall through to the default
    // no-op store and the user just gets defaults — no crash.
    register_local_storage_io();
    register_open_url();
    ensure_app();
    wasm_bindgen_futures::spawn_local(async {
        match init_wgpu_async().await {
            Ok(init) => WGPU_INIT.with(|c| *c.borrow_mut() = Some(init)),
            Err(err) => {
                web_sys::console::error_1(&JsValue::from_str(&format!("wgpu init failed: {err}")));
            }
        }
        mark_dirty();
    });
}

/// Open `url` in a new browser tab via `window.open(url, '_blank')`.
/// Wired to `solitaire_core::platform::request_open_url` so clicking
/// the GitHub source link in the About dialog opens in a new tab.
fn register_open_url() {
    solitaire_core::platform::set_open_url(|url| {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    });
}

/// Bridge `window.localStorage` into `solitaire_core::platform`'s
/// key/value hooks so `UserSettings` persists across reloads.
fn register_local_storage_io() {
    solitaire_core::platform::set_storage_io(
        |key| {
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item(key).ok().flatten())
        },
        |key, value| {
            if let Some(store) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                let _ = store.set_item(key, value);
            }
        },
    );
}

#[derive(Debug)]
struct WebDisplay;

impl wgpu::rwh::HasDisplayHandle for WebDisplay {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

async fn init_wgpu_async() -> Result<WgpuInit, String> {
    let document = web_sys::window()
        .ok_or("no global window")?
        .document()
        .ok_or("no document")?;
    let canvas = document
        .get_element_by_id("solitaire-canvas")
        .ok_or("#solitaire-canvas element not found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| "#solitaire-canvas is not a canvas")?;

    let mut instance_desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    instance_desc.backends = wgpu::Backends::GL;
    let instance = wgpu::Instance::new(instance_desc);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|err| format!("create_surface: {err:?}"))?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|err| format!("request_adapter: {err:?}"))?;

    // `downlevel_webgl2_defaults` pins `max_texture_dimension_2d = 2048`,
    // which any modern phone's `canvas_buffer = viewport × DPR` routinely
    // overshoots — e.g. a 1024×2217 CSS-px viewport at DPR 3 needs a
    // 3072×6651 surface and `Surface::configure` panics with a wgpu
    // validation error. `using_resolution(adapter.limits())` keeps every
    // other conservative WebGL2 default but raises the texture-dimension
    // caps to whatever the actual adapter advertises (typically 4096–
    // 16384 on Pixel-class hardware), so we can render at the real DPR
    // without the JS shell having to clamp the buffer down.
    let adapter_limits = adapter.limits();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("solitaire-wasm-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter_limits),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("request_device: {err:?}"))?;

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
        width: canvas.width().max(1),
        height: canvas.height().max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    Ok(WgpuInit {
        device: Arc::new(device),
        queue: Arc::new(queue),
        surface,
        surface_format,
        config,
    })
}

fn ensure_app() {
    APP.with(|cell| {
        if cell.borrow().is_some() {
            return;
        }
        *cell.borrow_mut() = Some(build_solitaire_app());
    });
}

fn ensure_wgpu_ctx(width: f32, height: f32) {
    WGPU_CTX.with(|ctx_cell| {
        if ctx_cell.borrow().is_some() {
            return;
        }
        WGPU_INIT.with(|init_cell| {
            let init = init_cell.borrow();
            let Some(init) = init.as_ref() else {
                return;
            };
            *ctx_cell.borrow_mut() = Some(WgpuGfxCtx::new(
                Arc::clone(&init.device),
                Arc::clone(&init.queue),
                init.surface_format,
                width,
                height,
            ));
        });
    });
}

fn resize_surface_if_needed(width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    WGPU_INIT.with(|cell| {
        let mut init = cell.borrow_mut();
        let Some(init) = init.as_mut() else {
            return;
        };
        if init.config.width != width || init.config.height != height {
            init.config.width = width;
            init.config.height = height;
            init.surface.configure(&init.device, &init.config);
        }
    });
}

#[wasm_bindgen]
pub fn render(width: u32, height: u32, _frame_ms: f64) {
    if !WGPU_INIT.with(|cell| cell.borrow().is_some()) {
        return;
    }
    ensure_app();
    ensure_wgpu_ctx(width as f32, height as f32);
    resize_surface_if_needed(width, height);

    let frame = WGPU_INIT.with(|cell| {
        let init = cell.borrow();
        let init = init.as_ref()?;
        match init.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            _ => None,
        }
    });
    let Some(frame) = frame else {
        return;
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    WGPU_CTX.with(|ctx_cell| {
        let mut ctx = ctx_cell.borrow_mut();
        let Some(ctx) = ctx.as_mut() else {
            return;
        };
        ctx.set_surface_texture(frame.texture.clone());
        ctx.reset(width as f32, height as f32);
        ctx.set_lcd_mode(agg_gui::font_settings::lcd_enabled());
        begin_frame(ctx, view);
        APP.with(|app_cell| {
            let mut app = app_cell.borrow_mut();
            if let Some(app) = app.as_mut() {
                app.layout(agg_gui::Size::new(width as f64, height as f64));
                app.paint(ctx);
            }
        });
        ctx.end_frame();
    });
    frame.present();
    NEEDS_DRAW.with(|cell| cell.set(false));
}

#[wasm_bindgen]
pub fn set_device_pixel_ratio(dpr: f64) {
    agg_gui::set_device_scale(dpr.max(0.5));
    agg_gui::font_settings::set_lcd_enabled(agg_gui::device_scale() <= 1.25);
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_move(x: f64, y: f64) {
    ensure_app();
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_move(x, y);
        }
    });
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_down(x: f64, y: f64, button: u8) {
    ensure_app();
    let btn = mouse_button(button);
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_down(x, y, btn, Modifiers::default());
        }
    });
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_up(x: f64, y: f64, button: u8) {
    ensure_app();
    let btn = mouse_button(button);
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_up(x, y, btn, Modifiers::default());
        }
    });
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_leave() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_leave();
        }
    });
    mark_dirty();
}

/// JS-side fullscreen toggle. Stored so the Rust menu handler can call
/// it via `solitaire_core::platform::request_toggle_fullscreen`. We
/// register a wrapper closure with the platform hook the first time
/// this gets called, and just overwrite the stored callback on
/// subsequent calls (Vite HMR / re-init).
#[wasm_bindgen]
pub fn register_fullscreen_toggle(cb: js_sys::Function) {
    use std::cell::RefCell;
    thread_local! {
        static FS_CB: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
        static REGISTERED: RefCell<bool> = const { RefCell::new(false) };
    }
    FS_CB.with(|cell| *cell.borrow_mut() = Some(cb));
    REGISTERED.with(|cell| {
        if *cell.borrow() {
            return;
        }
        *cell.borrow_mut() = true;
        solitaire_core::platform::set_fullscreen_toggle(|| {
            FS_CB.with(|cell| {
                if let Some(cb) = cell.borrow().as_ref() {
                    let _ = cb.call0(&JsValue::NULL);
                }
            });
        });
    });
}

#[wasm_bindgen]
pub fn needs_draw() -> bool {
    if NEEDS_DRAW.with(|cell| cell.get()) {
        return true;
    }
    APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|app| app.wants_draw())
            .unwrap_or(true)
    })
}

fn mouse_button(button: u8) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        other => MouseButton::Other(other),
    }
}

fn mark_dirty() {
    NEEDS_DRAW.with(|cell| cell.set(true));
}
