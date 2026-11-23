//! Window lifecycle for the desktop shell.
//!
//! The GUI runs as a regular app and is a view over the service: the window is
//! transient, and closing it leaves the GUI and the service running. The tray's
//! Show, the Dock reopen, and the single-instance callback are the restore paths.
//! Quit ends the GUI only; the service keeps serving.
//!
//! External links leave the window: the SPA origin keeps its navigations, and any
//! other http(s) URL goes to the OS default browser without opening a second
//! window.

use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::webview::{NewWindowFeatures, NewWindowResponse};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

#[cfg(test)]
#[path = "../test/lifecycle.rs"]
mod lifecycle_test;

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
/// Window in which a second report of the same external URL is the same click.
const OPEN_DEDUP_WINDOW: Duration = Duration::from_millis(200);

/// Last external URL handed to the OS, with the moment it happened.
static LAST_OPENED: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// How a navigation request is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationDecision {
    /// Keep the request in the window.
    Allow,
    /// Hand the URL to the operating system and keep the SPA where it is.
    OpenInBrowser,
}

/// Create the application window, loading the SPA at `route`.
///
/// No route means the SPA root. The route arrives as a launch argument from the
/// service tray, so Settings… can open the window on its page.
pub fn create_main_window(app: &AppHandle, route: Option<&str>) -> WindowResult<()> {
    create_window_at(app, route.unwrap_or(ROOT_PATH))
}

/// Create the application window and load the SPA at `path`.
///
/// Both navigation surfaces are hooked: a same-webview navigation keeps the SPA
/// origin and sends anything else to the OS browser, and a new-window request is
/// denied after the same handover, so a `target="_blank"` link never opens a
/// second window.
fn create_window_at(app: &AppHandle, path: &str) -> WindowResult<()> {
    let target = spa_url(path)?;
    WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::External(target))
        .title(MAIN_WINDOW_TITLE)
        .inner_size(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT)
        .on_navigation(|url| match navigation_decision(url) {
            NavigationDecision::Allow => true,
            NavigationDecision::OpenInBrowser => {
                open_in_os_browser(url);
                false
            }
        })
        .on_new_window(|url, _features: NewWindowFeatures| {
            open_in_os_browser(&url);
            NewWindowResponse::Deny
        })
        .build()?;
    Ok(())
}

/// Decide what to do with a navigation request.
///
/// The SPA origin serves every route the router knows, so it stays in the window.
/// Any other URL is a link the user wants in their own browser.
fn navigation_decision(url: &tauri::Url) -> NavigationDecision {
    if is_spa_origin(url) {
        NavigationDecision::Allow
    } else {
        NavigationDecision::OpenInBrowser
    }
}

/// Whether `url` sits on the origin this build serves the SPA from.
///
/// Compares scheme, host, and port, so any path and query on that origin passes.
fn is_spa_origin(url: &tauri::Url) -> bool {
    let Ok(origin) = tauri::Url::parse(&crate::server::spa_origin()) else {
        return false;
    };

    url.scheme() == origin.scheme()
        && url.host_str() == origin.host_str()
        && url.port_or_known_default() == origin.port_or_known_default()
}

/// Hand `url` to the OS default browser; anything not http(s) is ignored.
///
/// Returns whether a browser was started. The command runs detached: the window
/// never waits for the browser, and a failure leaves the SPA untouched.
fn open_in_os_browser(url: &tauri::Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }

    if !claim_open(url.as_str()) {
        return false;
    }

    Command::new("open").arg(url.as_str()).spawn().is_ok()
}

/// Whether this URL may open now, or already opened a moment ago.
///
/// WebKit can report one click through both policy hooks; the window turns that
/// pair into a single browser tab.
fn claim_open(url: &str) -> bool {
    let Ok(mut last) = LAST_OPENED.lock() else {
        return false;
    };

    if let Some((previous, at)) = last.as_ref()
        && previous == url
        && at.elapsed() < OPEN_DEDUP_WINDOW
    {
        return false;
    }

    *last = Some((url.to_string(), Instant::now()));
    true
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
    let origin: tauri::Url = crate::server::spa_origin().parse()?;
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
/// The GUI runs as a regular app, so a closed window leaves a Dock icon behind
/// and macOS sends `RunEvent::Reopen` when the user clicks it. The reopen brings
/// the window back without touching the service.
pub fn handle_run_event(app: &AppHandle, event: RunEvent) {
    match event {
        // Closing the last window must not stop the GUI: the Dock reopen brings
        // the view back, and the service keeps serving meanwhile.
        RunEvent::ExitRequested { code, api, .. } if code.is_none() => api.prevent_exit(),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => {
            if let Err(error) = show_window_at(app, ROOT_PATH) {
                eprintln!("context: failed to reopen the main window: {error}");
            }
        }
        _ => {}
    }
}
