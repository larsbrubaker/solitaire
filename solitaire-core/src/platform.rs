//! Platform-injected services.
//!
//! `solitaire-core` stays `wasm32`-clean (no `web_sys`, no `winit`), so any
//! action that needs to reach into the host (request fullscreen on a
//! browser, etc.) goes through a thread-local hook the platform shell
//! registers at startup. UI code calls the bare `request_*` function;
//! if no shell registered a handler (native build, headless tests),
//! the call is a no-op.

use std::cell::RefCell;

type ToggleFn = Box<dyn Fn()>;
type StorageLoadFn = Box<dyn Fn(&str) -> Option<String>>;
type StorageSaveFn = Box<dyn Fn(&str, &str)>;

thread_local! {
    static FULLSCREEN_TOGGLE: RefCell<Option<ToggleFn>> = const { RefCell::new(None) };
    static STORAGE_LOAD: RefCell<Option<StorageLoadFn>> = const { RefCell::new(None) };
    static STORAGE_SAVE: RefCell<Option<StorageSaveFn>> = const { RefCell::new(None) };
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

/// Register the platform's key/value storage backend. Used by
/// `AppModel` to persist the player's Options-menu choices across
/// launches. The WASM shell wires this up to `window.localStorage`;
/// native shells can plug in a file-backed store. With no backend
/// registered (default for headless tests) `storage_load` returns
/// `None` and `storage_save` is a no-op.
pub fn set_storage_io(
    load: impl Fn(&str) -> Option<String> + 'static,
    save: impl Fn(&str, &str) + 'static,
) {
    STORAGE_LOAD.with(|cell| *cell.borrow_mut() = Some(Box::new(load)));
    STORAGE_SAVE.with(|cell| *cell.borrow_mut() = Some(Box::new(save)));
}

/// Read a previously-stored value for `key`. `None` if absent OR if
/// no backend was registered.
pub fn storage_load(key: &str) -> Option<String> {
    STORAGE_LOAD.with(|cell| cell.borrow().as_ref().and_then(|f| f(key)))
}

/// Write `value` to `key`. Silently dropped if no backend was
/// registered.
pub fn storage_save(key: &str, value: &str) {
    STORAGE_SAVE.with(|cell| {
        if let Some(f) = cell.borrow().as_ref() {
            f(key, value);
        }
    });
}

/// Test-only: drop the registered storage backend so subsequent
/// `storage_load`/`storage_save` calls fall through to the default
/// no-op behaviour. Prevents thread-local state from one test from
/// leaking into another when cargo's test runner reuses threads.
#[cfg(test)]
pub fn clear_storage_io_for_test() {
    STORAGE_LOAD.with(|cell| *cell.borrow_mut() = None);
    STORAGE_SAVE.with(|cell| *cell.borrow_mut() = None);
}
