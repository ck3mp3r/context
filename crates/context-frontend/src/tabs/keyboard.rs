//! One window `keydown` listener: the cycle ring, the tab close, and the chrome shortcut.
//!
//! Decision 17: Cmd+W and Cmd+T arrive as a JS `keydown` in the webview. Nothing here goes through
//! a native menu item or an event bridge, and the frontend holds no `__TAURI__` reference at all.
//! macOS Cmd is `meta_key`; Cmd never sets `ctrl_key`, so the two combinations stay apart.
//!
//! Cmd+Shift+W is the native Close Window accelerator and is deliberately not handled here. Close a
//! tab, never the window (decision 9).

use leptos::ev;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use super::open::write_active_url;
use super::registry::TabRegistry;

// region:    --- Public functions

/// Install the one window `keydown` listener and return its handle.
///
/// The callback runs outside the reactive ownership tree, so the registry and the navigate function
/// are passed in rather than read from context. The caller removes the handle on cleanup.
pub fn start_keyboard(
    tabs: TabRegistry,
    navigate: impl Fn(&str, NavigateOptions) + 'static,
) -> WindowListenerHandle {
    window_event_listener(ev::keydown, move |event: ev::KeyboardEvent| {
        if event.repeat() {
            return;
        }
        if event.meta_key() && !event.ctrl_key() && !event.alt_key() {
            on_command(event, tabs, &navigate);
            return;
        }
        if event.ctrl_key() && !event.meta_key() && event.key() == "Tab" {
            event.prevent_default();
            if event.shift_key() {
                tabs.cycle_previous();
            } else {
                tabs.cycle_next();
            }
        }
    })
}

// endregion: --- Public functions

// region:    --- Support

/// Handle a plain Cmd chord: close the active tab, or reach the Projects chrome view.
///
/// Shift is excluded so Cmd+Shift+W stays the window's own accelerator, and Option is excluded so
/// the chords match exactly.
fn on_command(
    event: ev::KeyboardEvent,
    tabs: TabRegistry,
    navigate: &impl Fn(&str, NavigateOptions),
) {
    if event.shift_key() {
        return;
    }
    match event.key().to_ascii_lowercase().as_str() {
        "w" => {
            event.prevent_default();
            close_active(tabs);
        }
        "t" => {
            event.prevent_default();
            write_active_url(navigate, "/");
        }
        _ => {}
    }
}

/// Close the active tab. An empty strip has no active tab, so Cmd+W does nothing on chrome home.
fn close_active(tabs: TabRegistry) {
    if let Some(id) = tabs.active() {
        tabs.close(id);
    }
}

// endregion: --- Support
