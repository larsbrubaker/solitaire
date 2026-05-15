//! Persisted Options-menu choices.
//!
//! `AppModel` reads `UserSettings::load()` at construction and calls
//! `save()` whenever a settings-affecting setter runs. Storage goes
//! through the [`platform::storage_*`](crate::platform) hooks — WASM
//! uses `localStorage`, native shells could plug in a file. Settings
//! that affect *only* the runtime session (current screen, active
//! help dialog, in-flight game state) are NOT persisted here; only
//! choices the player makes via Options that should stick across
//! launches.

use serde::{Deserialize, Serialize};

use crate::cards::Suit;
use crate::platform;

/// Key used in the platform key/value store. Versioned so a future
/// format change can be detected via a parse failure.
const STORAGE_KEY: &str = "solitaire:settings:v1";

/// Persisted layout for the Debug → Performance window.
///
/// Stored as a flat record (not `agg_gui::geometry::Rect`) so the JSON
/// blob stays stable even if the geometry types shuffle.  All fields
/// are `f64` so position / size restore exactly across launches.
/// `visible` records the last open/closed state of the window so
/// devs who left it open across a session don't have to re-open it
/// after restarting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerfWindowState {
    pub visible: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for PerfWindowState {
    fn default() -> Self {
        // Match the spawn position used by `build_performance_window`
        // so first-launch and "restored from defaults" produce the
        // same window placement.
        Self {
            visible: false,
            x: 60.0,
            y: 60.0,
            width: 320.0,
            height: 168.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    pub klondike_draw_count: u8,
    pub spider_suit_count: u8,
    pub spider_one_suit: Suit,
    /// When true, Spider new-deal picks a seed from the bundled list
    /// of solver-verified winnable deals instead of a wallclock seed.
    /// Default `true` — winnable-only is the welcoming choice; players
    /// who want hard mode can flip it off in Options.
    #[serde(default = "default_true")]
    pub spider_winnable_only: bool,
    /// When true, FreeCell new-deal reproduces a Microsoft FreeCell
    /// game number drawn from the original 32,000-deal pool
    /// (skipping the known-unwinnable #11982). Default `true`.
    #[serde(default = "default_true")]
    pub freecell_winnable_only: bool,
    /// When true, Klondike new-deal picks a seed from the bundled
    /// Solvitaire-verified winnable list. Default `true`.
    #[serde(default = "default_true")]
    pub klondike_winnable_only: bool,
    /// Persisted Debug → Performance window state (visible + last
    /// position/size).  `serde(default)` keeps stored blobs from
    /// pre-`PerfWindowState` versions parseable.
    #[serde(default)]
    pub perf_window: PerfWindowState,
}

/// Serde helper for fields that should default to `true` when an
/// older saved settings blob is missing them. (Bare `#[serde(default)]`
/// would pick `bool::default() == false` — wrong for our
/// winnable-only fields where the product-level default is "on".)
fn default_true() -> bool {
    true
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            klondike_draw_count: 1,
            spider_suit_count: 1,
            spider_one_suit: Suit::Spades,
            spider_winnable_only: true,
            freecell_winnable_only: true,
            klondike_winnable_only: true,
            perf_window: PerfWindowState::default(),
        }
    }
}

impl UserSettings {
    /// Read persisted settings (or defaults when nothing's stored or
    /// the stored JSON fails to parse — a forward-incompat reset).
    pub fn load() -> Self {
        let Some(s) = platform::storage_load(STORAGE_KEY) else {
            return Self::default();
        };
        serde_json::from_str(&s).unwrap_or_default()
    }

    /// Write current settings to the platform's key/value store.
    /// Failures (no backend registered, full storage, etc.) are
    /// silently dropped — settings are a "nice to have", not a
    /// correctness requirement.
    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string(self) {
            platform::storage_save(STORAGE_KEY, &s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    /// RAII guard around an in-memory storage backend. Installs a
    /// fresh empty backend on construction; clears the platform's
    /// thread-local hooks on drop so the next test on this thread
    /// starts from a clean state (cargo's test runner reuses threads,
    /// so without this a test that wrote draw_count=3 would poison a
    /// later test's `AppModel::new()` default expectations).
    struct StorageGuard {
        _store: Rc<RefCell<HashMap<String, String>>>,
    }
    impl Drop for StorageGuard {
        fn drop(&mut self) {
            platform::clear_storage_io_for_test();
        }
    }

    fn install_test_storage() -> StorageGuard {
        let store = Rc::new(RefCell::new(HashMap::new()));
        let load_store = store.clone();
        let save_store = store.clone();
        platform::set_storage_io(
            move |k| load_store.borrow().get(k).cloned(),
            move |k, v| {
                save_store.borrow_mut().insert(k.to_string(), v.to_string());
            },
        );
        StorageGuard { _store: store }
    }

    #[test]
    fn defaults_when_nothing_stored() {
        let _guard = install_test_storage();
        let s = UserSettings::load();
        assert_eq!(s, UserSettings::default());
        assert_eq!(s.spider_suit_count, 1);
        assert_eq!(s.spider_one_suit, Suit::Spades);
    }

    #[test]
    fn round_trips_through_storage() {
        let _guard = install_test_storage();
        let s = UserSettings {
            klondike_draw_count: 3,
            spider_suit_count: 1,
            spider_one_suit: Suit::Hearts,
            spider_winnable_only: true,
            freecell_winnable_only: false,
            klondike_winnable_only: false,
            perf_window: PerfWindowState {
                visible: true,
                x: 100.0,
                y: 200.0,
                width: 400.0,
                height: 220.0,
            },
        };
        s.save();
        assert_eq!(UserSettings::load(), s);
    }

    #[test]
    fn garbage_storage_resets_to_defaults() {
        let _guard = install_test_storage();
        platform::storage_save(STORAGE_KEY, "{not json}");
        assert_eq!(UserSettings::load(), UserSettings::default());
    }
}
