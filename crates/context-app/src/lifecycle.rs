//! Window lifecycle for the desktop shell.
//!
//! The app runs as a macOS accessory agent: it has no Dock icon and no Cmd+Tab
//! entry, and the menu bar belongs to the frontmost regular app. The window is a
//! transient view. Closing it leaves the app and the embedded server running,
//! because MCP clients keep using the server while the window is closed. The
//! tray's Show and the single-instance callback are the restore paths. The tray
//! owns the only process exit; every window-closing surface — the application-menu
//! item, Cmd+Q, Cmd+W, the close button — only closes the view.

use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Error raised while managing the application window.
///
/// The setup hook of `tauri::Builder` reports failures as boxed errors.
type WindowResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Label of the application window.
const MAIN_WINDOW_LABEL: &str = "main";
/// Title of the application window.
const MAIN_WINDOW_TITLE: &str = "context";
/// Initial width of the application window in logical pixels.
const MAIN_WINDOW_WIDTH: f64 = 1280.0;
/// Initial height of the application window in logical pixels.
const MAIN_WINDOW_HEIGHT: f64 = 800.0;
/// SPA root path: showing it keeps the route the webview already has.
const ROOT_PATH: &str = "/";
/// SPA path of the settings page, the tray's deep link target.
pub const SETTINGS_PATH: &str = "/settings";

/// Create the application window and load the SPA at its root.
pub fn create_main_window(app: &AppHandle) -> WindowResult<()> {
    create_window_at(app, ROOT_PATH)
}

/// Create the application window and load the SPA at `path`.
fn create_window_at(app: &AppHandle, path: &str) -> WindowResult<()> {
    let target = spa_url(path)?;
    WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::External(target))
        .title(MAIN_WINDOW_TITLE)
        .inner_size(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT)
        .build()?;
    Ok(())
}

/// Show and focus the application window, creating it when it was closed.
///
/// A `path` other than the root navigates the SPA there, so a deep link lands on
/// its page whether the window already existed or was just created. The root
/// keeps the route the webview already had.
fn show_window_at(app: &AppHandle, path: &str) -> WindowResult<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return create_window_at(app, path);
    };

    window.unminimize()?;
    window.show()?;
    window.set_focus()?;

    if path != ROOT_PATH {
        window.navigate(spa_url(path)?)?;
    }

    Ok(())
}

/// SPA URL of `path` on the origin this build loads.
///
/// The origin ends in a slash and `join` resolves the path against it, so the
/// deep link is a single-slash URL on whichever origin serves the SPA.
pub(crate) fn spa_url(path: &str) -> WindowResult<tauri::Url> {
    let origin: tauri::Url = crate::spa_origin().parse()?;
    Ok(origin.join(path.trim_start_matches('/'))?)
}

/// Close the application window, leaving the app and the server running.
///
/// `WebviewWindow::close` is the graceful close: it raises the close-requested
/// event, the same path the predefined close-window item and the close button
/// take. Closing the last window raises `ExitRequested { code: None }`, which
/// `handle_run_event` vetoes, so the process survives. A closed window is not an
/// error — the tray brings it back.
pub fn close_main_window(app: &AppHandle) -> WindowResult<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };

    window.close()?;
    Ok(())
}

/// Show the application window on the main thread.
///
/// The single-instance plugin calls its callback on a worker thread, and window
/// operations belong to the main thread.
pub fn request_main_window(app: &AppHandle) {
    request_window_at(app, ROOT_PATH);
}

/// Show the window on the main thread, navigating the SPA to `path`.
///
/// The create-or-show and the navigation run in the same main-thread step, so a
/// deep link lands on its page even when the window had to be created first.
pub fn request_window_at(app: &AppHandle, path: &str) {
    let scheduler = app.clone();
    let app = app.clone();
    let path = path.to_string();
    if let Err(error) = scheduler.run_on_main_thread(move || {
        if let Err(error) = show_window_at(&app, &path) {
            eprintln!("context: failed to show the main window: {error}");
        }
    }) {
        eprintln!("context: failed to schedule the main window: {error}");
    }
}

/// Handle the application events that affect the window lifecycle.
///
/// The app is an accessory agent, so macOS never sends `RunEvent::Reopen`: that
/// event comes from the Dock's reopen callback, and there is no Dock icon.
pub fn handle_run_event(_app: &AppHandle, event: RunEvent) {
    match event {
        // Closing the last window must not stop the app: the embedded server
        // keeps serving MCP clients until the user quits.
        RunEvent::ExitRequested { code, api, .. } if code.is_none() => api.prevent_exit(),
        _ => {}
    }
}
