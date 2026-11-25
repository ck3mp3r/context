//! The one native seam: the window title follows the active tab.
//!
//! Decision P, and decision 18 for the degraded case: the SPA names the window after the active tab
//! through one `setTitle` call, and nothing else. There is no event bridge, no `listen()`, and no
//! native menu item behind it. The call is best-effort — a webview without the injected
//! `__TAURI__` global keeps the static window title, and no interaction depends on the call.
//!
//! The only IPC this frontend holds is `core:window:allow-set-title` for the two SPA origins
//! (`crates/context-app/capabilities/spa-ipc.json`).

use js_sys::{Function, Reflect};
use leptos::prelude::Effect;
use wasm_bindgen::{JsCast, JsValue};

use super::registry::TabRegistry;
use super::session::TabTitles;
use super::strip::tab_label;

// region:    --- Types

/// The window title on chrome, and the prefix of every title a tab names.
pub const APP_TITLE: &str = "Context";
/// The global Tauri injects when `app.withGlobalTauri` is on.
const TAURI_GLOBAL: &str = "__TAURI__";
/// Between the application name and the active tab's label.
const TITLE_SEPARATOR: &str = " — ";

// endregion: --- Types

// region:    --- Public functions

/// Install the effect that names the window after the active tab.
///
/// One effect over the registry and the labels, so every activation, close, and label resolve sets
/// the title once. Chrome home names the plain application title.
pub fn start_window_title(tabs: TabRegistry, titles: TabTitles) {
    Effect::new(move |_| set_window_title(&window_title(tabs, titles)));
}

/// Set the window title, or nothing when the webview has no Tauri global.
///
/// Best-effort by decision 18: a missing global, a missing method, and a rejected call all leave
/// the static title in place, so the shell never reports a failure it cannot act on.
pub fn set_window_title(title: &str) {
    let Some(window) = current_window() else {
        return;
    };
    let Some(set_title) = function_field(&window, "setTitle") else {
        return;
    };
    let _ = set_title.call1(&window, &JsValue::from_str(title));
}

// endregion: --- Public functions

// region:    --- Support

/// The title the active tab names, or the plain application title on chrome.
///
/// The label rule is the strip's, so the window and the chip never disagree about a tab.
fn window_title(tabs: TabRegistry, titles: TabTitles) -> String {
    match tabs.active_key() {
        Some(key) => format!("{APP_TITLE}{TITLE_SEPARATOR}{}", tab_label(&titles, &key)),
        None => APP_TITLE.to_string(),
    }
}

/// The Tauri window object of this page, or `None` when the global is absent.
fn current_window() -> Option<JsValue> {
    let window = web_sys::window()?;
    let tauri = Reflect::get(window.as_ref(), &JsValue::from_str(TAURI_GLOBAL)).ok()?;
    if !tauri.is_object() {
        return None;
    }
    let tauri_window = Reflect::get(&tauri, &JsValue::from_str("window")).ok()?;
    let get_current_window = function_field(&tauri_window, "getCurrentWindow")?;
    get_current_window.call0(&tauri_window).ok()
}

/// The named property of `object`, when it is a function.
fn function_field(object: &JsValue, name: &str) -> Option<Function> {
    Reflect::get(object, &JsValue::from_str(name))
        .ok()?
        .dyn_into::<Function>()
        .ok()
}

// endregion: --- Support
