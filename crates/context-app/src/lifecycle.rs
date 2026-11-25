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
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::webview::{NewWindowFeatures, NewWindowResponse, PageLoadEvent};
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
/// Title of the application window: the default the SPA overrides per active tab.
///
/// The SPA names the window after the active tab through one `setTitle` call
/// (decision P). A webview without the Tauri global never makes that call, so
/// this static title stays, and no interaction depends on it.
const MAIN_WINDOW_TITLE: &str = "Context";
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
/// Delay between SPA origin probes of the recovery watch.
const RECOVERY_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How long the first navigation may stay uncommitted before the watchdog acts.
const RECOVERY_LOAD_BOUND: Duration = Duration::from_secs(15);
/// How many reloads the first-load watchdog may schedule for one window.
const RECOVERY_RELOAD_LIMIT: usize = 3;
/// How long the paint probe may take before the watchdog calls it a miss.
///
/// A live webview answers in milliseconds. A webview that is still pending drops
/// the eval callback instead of answering, so the wait has to end on its own.
const PAINT_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
/// JavaScript the watchdog evaluates to read the SPA's painted marker.
///
/// `<main>` is the SPA's root element: the shell renders it, so its presence
/// means the WebAssembly booted. The answer comes back through the eval
/// callback, so no bridge, no capability, and no SPA change are involved.
const PAINT_PROBE: &str = "!!document.querySelector('main')";

/// Last external URL handed to the OS, with the moment it happened.
static LAST_OPENED: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// Whether the main window ever reported a committed navigation.
///
/// A commit is not a paint: `index.html` commits as soon as it is served, so this
/// latch says nothing about whether the SPA rendered. It only tells the watchdog
/// that a paint probe is worth sending; it never suppresses the reload.
static COMMIT_SEEN: AtomicBool = AtomicBool::new(false);

/// Whether the SPA confirmed it painted its root element.
///
/// Latched: once a probe answers, the watchdog stops probing and stays quiet for
/// the rest of the process.
static PAINT_CONFIRMED: AtomicBool = AtomicBool::new(false);

/// Whether a reload is scheduled and has not run yet.
///
/// One incident reloads once: the flag absorbs every detector that fires while
/// a reload is pending, and clears when the scheduled reload runs.
static RELOAD_PENDING: AtomicBool = AtomicBool::new(false);

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
///
/// The new window is focused before returning: creating a window only orders it
/// within this app, so an inactive app would leave the recreated window behind
/// the frontmost one.
fn create_window_at(app: &AppHandle, path: &str) -> WindowResult<()> {
    let target = spa_url(path)?;
    let window = WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::External(target))
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
        .on_page_load(|_window, payload| {
            if payload.event() == PageLoadEvent::Started {
                COMMIT_SEEN.store(true, Ordering::SeqCst);
            }
        })
        .build()?;

    window.set_focus()?;
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
/// keeps the route the webview already had. Both branches end with the window
/// focused: a recreated window is activated by `create_window_at`, so the restore
/// paths bring it to the foreground either way.
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
/// event, the same path the Close Window item and the close button take. Closing
/// the last window raises `ExitRequested { code: None }`, which
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

/// Whether an origin probe pair is the recovery that needs one reload.
///
/// Only the DOWN→UP transition reloads: a reachable origin stays quiet, and a
/// lost origin waits for its recovery.
pub(crate) fn reload_on_recovery(previous_up: bool, now_up: bool) -> bool {
    !previous_up && now_up
}

/// What the first-load watchdog knows about the main window at one poll.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WatchdogState {
    /// Whether the window ever reported a committed navigation. It decides
    /// whether a paint probe is worth sending, not whether to reload.
    pub(crate) committed: bool,
    /// Whether the window url names the origin this build serves the SPA from.
    pub(crate) url_names_origin: bool,
    /// Whether the SPA confirmed it painted its root element.
    pub(crate) painted: bool,
    /// Whether the SPA origin answers.
    pub(crate) origin_up: bool,
    /// Reloads this watchdog already scheduled for the window.
    pub(crate) reloads: usize,
    /// Time since the window was created.
    pub(crate) elapsed: Duration,
}

/// What the first-load watchdog decided for one poll of the main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WatchdogVerdict {
    /// Leave the window alone.
    Idle,
    /// The first load never finished: reload the window.
    Reload,
}

/// Decide whether the first-load watchdog reloads the window on this poll.
///
/// A committed navigation is not a painted one: `index.html` answers 200 and
/// commits long before the SPA's WebAssembly renders, and the window url already
/// names the origin by then. Commit and url therefore take no part in the
/// decision — only `painted` does. An unconfirmed paint, including a probe that
/// never answers, reloads, so the watchdog fails toward a reload and never
/// toward a blank window.
pub(crate) fn watchdog_verdict(state: &WatchdogState) -> WatchdogVerdict {
    let due = state.origin_up
        && state.reloads < RECOVERY_RELOAD_LIMIT
        && state.elapsed >= RECOVERY_LOAD_BOUND;
    if !due || state.painted {
        return WatchdogVerdict::Idle;
    }

    WatchdogVerdict::Reload
}

/// Whether the watchdog should ask the webview to confirm a paint.
///
/// Only a page that committed on the origin this build serves can confirm a
/// paint, and a paint already confirmed needs no second question. Probing any
/// other window would spend an eval on every poll for no answer.
pub(crate) fn probe_due(state: &WatchdogState) -> bool {
    state.committed && state.url_names_origin && !state.painted
}

/// What the paint probe reported about the SPA root element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaintProbe {
    /// The SPA confirmed it painted its root element.
    Painted,
    /// The webview answered, and the SPA had not painted its root element.
    Blank,
    /// The webview never answered: the eval failed, or the pending webview
    /// dropped the callback. Counts as not painted.
    Unanswered,
    /// The probe was not sent: nothing committed on this origin, so there is no
    /// page of ours that could confirm a paint.
    Skipped,
}

/// Read a paint probe answer.
///
/// The eval callback hands back the value serialized as JSON, so a painted page
/// answers `true` and a blank one `false`. An empty answer means the expression
/// produced nothing. Only `true` counts as painted; every other answer is a
/// miss, so the watchdog fails toward reloading.
pub(crate) fn paint_answer(answer: &str) -> PaintProbe {
    match answer.trim().trim_matches('"') {
        "true" => PaintProbe::Painted,
        "false" => PaintProbe::Blank,
        _ => PaintProbe::Unanswered,
    }
}

/// Ask the webview whether the SPA painted its root element.
///
/// `eval_with_callback` is the only read-back: `eval` is fire-and-forget and
/// cannot report a value. The callback carries the answer, and the wait ends at
/// [`PAINT_PROBE_TIMEOUT`] whether or not it arrives — a webview that is still
/// pending drops the callback, and an `Err` means the window is gone. Both count
/// as not painted.
async fn confirm_painted(app: &AppHandle) -> PaintProbe {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return PaintProbe::Unanswered;
    };

    let (answers, mut answer) = tokio::sync::mpsc::unbounded_channel();
    if window
        .eval_with_callback(PAINT_PROBE, move |reply| {
            let _ = answers.send(reply);
        })
        .is_err()
    {
        return PaintProbe::Unanswered;
    }

    match tokio::time::timeout(PAINT_PROBE_TIMEOUT, answer.recv()).await {
        Ok(Some(reply)) => paint_answer(&reply),
        _ => PaintProbe::Unanswered,
    }
}

/// Start the loop that restores the main window after a failed first load or an
/// origin outage.
///
/// One loop carries two detectors. The origin watcher reloads on the DOWN→UP
/// transition of the origin this build loads. The first-load watchdog reloads a
/// window that never confirmed it painted while the origin answers, so neither a
/// silently dropped first navigation nor a committed-but-blank page leaves a
/// blank view; it acts at most [`RECOVERY_RELOAD_LIMIT`] times and stays quiet
/// once the SPA confirms a paint. Both detectors share [`schedule_reload`], so
/// one incident reloads once.
pub fn spawn_recovery_watch(app: &AppHandle) {
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        let origin = crate::server::spa_origin();
        let created = Instant::now();
        let mut previous_up = crate::server::origin_ready(&origin).await;
        let mut watchdog_reloads = 0_usize;

        loop {
            tokio::time::sleep(RECOVERY_POLL_INTERVAL).await;

            let now_up = crate::server::origin_ready(&origin).await;
            if reload_on_recovery(previous_up, now_up) {
                schedule_reload(
                    &app,
                    "context: reloaded the main window after the spa origin recovered".to_string(),
                );
            }
            previous_up = now_up;

            let watchdog_due = now_up
                && watchdog_reloads < RECOVERY_RELOAD_LIMIT
                && !PAINT_CONFIRMED.load(Ordering::SeqCst)
                && created.elapsed() >= RECOVERY_LOAD_BOUND;
            if watchdog_due {
                let url = main_window_url(&app);
                let observed = WatchdogState {
                    committed: COMMIT_SEEN.load(Ordering::SeqCst),
                    url_names_origin: matches!(&url, Ok(url) if is_spa_origin(url)),
                    painted: PAINT_CONFIRMED.load(Ordering::SeqCst),
                    origin_up: now_up,
                    reloads: watchdog_reloads,
                    elapsed: created.elapsed(),
                };

                let probe = if probe_due(&observed) {
                    confirm_painted(&app).await
                } else {
                    PaintProbe::Skipped
                };
                let painted = probe == PaintProbe::Painted;
                if painted {
                    PAINT_CONFIRMED.store(true, Ordering::SeqCst);
                }

                let state = WatchdogState {
                    painted,
                    ..observed
                };

                if watchdog_verdict(&state) == WatchdogVerdict::Reload {
                    watchdog_reloads += 1;
                    let reported = match &url {
                        Ok(url) => url.to_string(),
                        Err(error) => format!("<{error}>"),
                    };
                    schedule_reload(
                        &app,
                        format!(
                            "context: reloaded the main window after the first load stayed blank (url={reported:?}, paint={probe:?}, attempt={watchdog_reloads}/{RECOVERY_RELOAD_LIMIT})"
                        ),
                    );
                }
            }
        }
    });
}

/// Reload the main window after its web content process terminated.
///
/// The hook hands over the webview that died, so the reload is scheduled onto
/// the main thread. A terminated process leaves a blank window, so this path
/// reloads on its own instead of sharing the watchdog's pending flag.
pub fn reload_after_web_content_terminate(webview: &tauri::Webview) {
    let scheduler = webview.clone();
    let webview = webview.clone();
    let scheduled = scheduler.run_on_main_thread(move || match webview.reload() {
        Ok(()) => {
            eprintln!("context: reloaded the main window after the web content process terminated")
        }
        Err(error) => eprintln!("context: failed to reload the main window: {error}"),
    });

    if let Err(error) = scheduled {
        eprintln!("context: failed to schedule the main window reload: {error}");
    }
}

/// Schedule one reload of the main window on the main thread.
///
/// `line` is the stderr diagnostic the reload reports on success. The pending
/// flag absorbs every later request until the scheduled reload runs, so the
/// origin watcher and the watchdog share one reload per incident.
fn schedule_reload(app: &AppHandle, line: String) {
    if RELOAD_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }

    let scheduler = app.clone();
    let app = app.clone();
    let scheduled = scheduler.run_on_main_thread(move || {
        RELOAD_PENDING.store(false, Ordering::SeqCst);

        let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
            eprintln!("context: skipped the main window reload: the window is gone");
            return;
        };

        match window.reload() {
            Ok(()) => eprintln!("{line}"),
            Err(error) => eprintln!("context: failed to reload the main window: {error}"),
        }
    });

    if let Err(error) = scheduled {
        RELOAD_PENDING.store(false, Ordering::SeqCst);
        eprintln!("context: failed to schedule the main window reload: {error}");
    }
}

/// Current URL of the main window, as the first-load watchdog reports it.
fn main_window_url(app: &AppHandle) -> Result<tauri::Url, String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "the main window is gone".to_string())?;

    window.url().map_err(|error| error.to_string())
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
