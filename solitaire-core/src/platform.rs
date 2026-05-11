//! Platform-injected services.
//!
//! `solitaire-core` stays `wasm32`-clean (no `web_sys`, no `winit`), so any
//! action that needs to reach into the host (request fullscreen on a
//! browser, etc.) goes through a thread-local hook the platform shell
//! registers at startup. UI code calls the bare `request_*` function;
//! if no shell registered a handler (native build, headless tests),
//! the call is a no-op.

use std::cell::RefCell;

thread_local! {
    static FULLSCREEN_TOGGLE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Register the platform's fullscreen toggle implementation. Called
/// once from the platform shell (e.g. `solitaire-wasm`) at startup.
pub fn set_fullscreen_toggle(f: impl Fn() + 'static) {
    FULLSCREEN_TOGGLE.with(|cell| *cell.borrow_mut() = Some(Box::new(f)));
}

/// Ask the host to toggle fullscreen. No-op if no platform shell
/// registered a handler.
pub fn request_toggle_fullscreen() {
    FULLSCREEN_TOGGLE.with(|cell| {
        if let Some(f) = cell.borrow().as_ref() {
            f();
        }
    });
}
