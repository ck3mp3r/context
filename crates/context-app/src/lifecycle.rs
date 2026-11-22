//! Window lifecycle for the desktop shell.
//!
//! The app owns one window. Closing it leaves the app and the embedded server
//! running, because MCP clients keep using the server while the window is
//! closed. Activating the app again shows a window; quitting ends the process.

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

/// Create the application window and load the SPA.
pub fn create_main_window(app: &AppHandle) -> WindowResult<()> {
    let origin: tauri::Url = crate::spa_origin().parse()?;
    WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::External(origin))
        .title(MAIN_WINDOW_TITLE)
        .inner_size(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT)
        .build()?;
    Ok(())
}

/// Show and focus the application window, creating it when it was closed.
pub fn show_main_window(app: &AppHandle) -> WindowResult<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return create_main_window(app);
    };

    window.unminimize()?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

/// Show the application window on the main thread.
///
/// The single-instance plugin calls its callback on a worker thread, and window
/// operations belong to the main thread.
pub fn request_main_window(app: &AppHandle) {
    let scheduler = app.clone();
    let app = app.clone();
    if let Err(error) = scheduler.run_on_main_thread(move || {
        if let Err(error) = show_main_window(&app) {
            eprintln!("context: failed to show the main window: {error}");
        }
    }) {
        eprintln!("context: failed to schedule the main window: {error}");
    }
}

/// Handle the application events that affect the window lifecycle.
pub fn handle_run_event(app: &AppHandle, event: RunEvent) {
    match event {
        // Closing the last window must not stop the app: the embedded server
        // keeps serving MCP clients until the user quits.
        RunEvent::ExitRequested { code, api, .. } if code.is_none() => api.prevent_exit(),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => request_main_window(app),
        _ => {}
    }
}
