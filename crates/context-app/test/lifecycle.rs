//! Tests for the window lifecycle: the external-link navigation decision, the
//! open-once guard that both policy hooks share, and the recovery watch helpers.

use std::io::{Read, Write};
use std::net::TcpListener;

use super::*;
use context_core::{clear_base_path, set_base_path};
use serial_test::serial;
use tempfile::TempDir;

/// Answer one request with `response`, then let the listener drop.
fn answer_once(listener: TcpListener, response: &'static [u8]) {
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(response);
        }
    });
}

/// A listener on a free loopback port, plus its URL.
fn origin_url() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind should succeed");
    let port = listener
        .local_addr()
        .expect("Local addr should exist")
        .port();
    let url = format!("http://127.0.0.1:{port}/");

    (listener, url)
}

/// Base path this test alone owns, so the SPA origin is the profile default.
fn base() -> TempDir {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());
    base
}

fn spa_url_of(path: &str) -> tauri::Url {
    let origin = tauri::Url::parse(&crate::server::spa_origin()).expect("Origin should parse");
    origin.join(path).expect("Route should join")
}

#[test]
#[serial]
fn spa_origin_keeps_the_navigation() {
    let base = base();

    let decision = navigation_decision(&spa_url_of(""));

    clear_base_path();
    drop(base);

    assert_eq!(decision, NavigationDecision::Allow);
}

#[test]
#[serial]
fn spa_route_keeps_the_navigation() {
    let base = base();
    let url = spa_url_of("settings?tab=advanced#top");

    let decision = navigation_decision(&url);

    clear_base_path();
    drop(base);

    assert_eq!(
        decision,
        NavigationDecision::Allow,
        "a deep link stays on the SPA origin: {url}"
    );
}

#[test]
#[serial]
fn external_https_link_opens_in_the_browser() {
    let base = base();
    let url = tauri::Url::parse("https://github.com/x").expect("URL should parse");

    let decision = navigation_decision(&url);

    clear_base_path();
    drop(base);

    assert_eq!(decision, NavigationDecision::OpenInBrowser);
}

#[test]
#[serial]
fn external_http_link_opens_in_the_browser() {
    let base = base();
    let url = tauri::Url::parse("http://example.com/page").expect("URL should parse");

    let decision = navigation_decision(&url);

    clear_base_path();
    drop(base);

    assert_eq!(decision, NavigationDecision::OpenInBrowser);
}

#[test]
fn non_http_scheme_never_spawns_the_browser() {
    for raw in [
        "mailto:someone@example.com",
        "file:///etc/hosts",
        "data:text/plain,hi",
    ] {
        let url = tauri::Url::parse(raw).expect("URL should parse");

        assert!(
            !open_in_os_browser(&url),
            "{raw} must not reach the OS opener"
        );
    }
}

#[test]
fn the_same_url_opens_once_within_the_window() {
    assert!(
        claim_open("https://example.com/same"),
        "the first hit opens"
    );
    assert!(
        !claim_open("https://example.com/same"),
        "the second hook for the same click must stay silent"
    );
}

#[test]
fn different_urls_each_open() {
    assert!(claim_open("https://example.com/one"));
    assert!(claim_open("https://example.com/two"));
}

#[test]
fn reload_on_recovery_fires_only_on_the_down_to_up_transition() {
    assert!(
        reload_on_recovery(false, true),
        "a recovered origin needs one reload"
    );
    assert!(
        !reload_on_recovery(true, true),
        "a reachable origin must not reload every poll"
    );
    assert!(
        !reload_on_recovery(false, false),
        "a down origin waits for the recovery"
    );
    assert!(
        !reload_on_recovery(true, false),
        "losing the origin is not a reload trigger"
    );
}

/// Every input the first-load watchdog weighs for one due poll.
fn state(
    committed: bool,
    url_names_origin: bool,
    painted: bool,
    origin_up: bool,
    reloads: usize,
) -> WatchdogState {
    WatchdogState {
        committed,
        url_names_origin,
        painted,
        origin_up,
        reloads,
        elapsed: RECOVERY_LOAD_BOUND,
    }
}

#[test]
fn a_committed_but_blank_spa_reloads() {
    // The owner's regression: `index.html` commits and the window url names the
    // origin, yet the SPA never paints. Commit and url are not a paint, so the
    // watchdog must reload instead of leaving a blank window.
    let decided = watchdog_verdict(&state(true, true, false, true, 0));

    assert_eq!(
        decided,
        WatchdogVerdict::Reload,
        "a committed but never painted SPA must reload"
    );
}

#[test]
fn a_painted_spa_stays_quiet() {
    let decided = watchdog_verdict(&state(true, true, true, true, 0));

    assert_eq!(
        decided,
        WatchdogVerdict::Idle,
        "a confirmed paint must not reload"
    );
}

#[test]
fn an_unconfirmed_paint_reloads() {
    // The probe never answered (`eval_with_callback` returned `Err`, or the
    // webview dropped the callback): the watchdog fails toward reloading.
    let decided = watchdog_verdict(&state(false, false, false, true, 0));

    assert_eq!(
        decided,
        WatchdogVerdict::Reload,
        "a failed first navigation must still reload"
    );
}

#[test]
fn the_watchdog_waits_for_the_load_bound() {
    let mut early = state(true, true, false, true, 0);
    early.elapsed = RECOVERY_LOAD_BOUND - Duration::from_millis(1);

    assert_eq!(
        watchdog_verdict(&early),
        WatchdogVerdict::Idle,
        "an unelapsed bound must not reload"
    );
}

#[test]
fn the_watchdog_stops_at_the_reload_limit() {
    let spent = state(true, true, false, true, RECOVERY_RELOAD_LIMIT);

    assert_eq!(
        watchdog_verdict(&spent),
        WatchdogVerdict::Idle,
        "the reload budget is finite"
    );
}

#[test]
fn a_down_origin_leaves_the_reload_to_the_origin_watcher() {
    let down = state(true, true, false, false, 0);

    assert_eq!(
        watchdog_verdict(&down),
        WatchdogVerdict::Idle,
        "the origin watcher owns the recovered origin reload"
    );
}

#[test]
fn the_probe_asks_only_a_committed_page_on_the_origin() {
    assert!(
        probe_due(&state(true, true, false, true, 0)),
        "a committed page on the origin is the only page that can confirm a paint"
    );
    assert!(
        !probe_due(&state(false, false, false, true, 0)),
        "nothing committed: there is no page to ask"
    );
    assert!(
        !probe_due(&state(true, true, true, true, 0)),
        "a confirmed paint needs no second question"
    );
    assert!(
        !probe_due(&state(true, false, false, true, 0)),
        "a window off the origin has nothing of ours to probe"
    );
}

#[test]
fn only_an_explicit_true_confirms_the_paint() {
    assert_eq!(paint_answer("true"), PaintProbe::Painted);
    assert_eq!(paint_answer("true\n"), PaintProbe::Painted);
    assert_eq!(paint_answer("\"true\""), PaintProbe::Painted);
    assert_eq!(paint_answer("false"), PaintProbe::Blank);
    assert_eq!(
        paint_answer(""),
        PaintProbe::Unanswered,
        "an empty answer is a miss, never a confirmation"
    );
    assert_eq!(
        paint_answer("null"),
        PaintProbe::Unanswered,
        "any other answer is a miss, never a confirmation"
    );
}

#[test]
#[serial]
fn a_new_window_starts_with_fresh_watchdog_state() {
    PAINT_CONFIRMED.store(true, Ordering::SeqCst);
    WATCHDOG_RELOADS.store(RECOVERY_RELOAD_LIMIT, Ordering::SeqCst);
    *WINDOW_CREATED
        .lock()
        .expect("The creation clock lock should be held") =
        Some(Instant::now() - Duration::from_secs(100));

    reset_watchdog_state();

    assert!(
        !PAINT_CONFIRMED.load(Ordering::SeqCst),
        "a new window must be probed again instead of inheriting a paint"
    );
    assert_eq!(
        WATCHDOG_RELOADS.load(Ordering::SeqCst),
        0,
        "a new window must not inherit a spent reload budget"
    );
    assert!(
        window_age() < Duration::from_secs(1),
        "a new window must count its load bound from its own creation"
    );
}

#[test]
#[serial]
fn a_recreated_window_resets_the_paint_latch() {
    PAINT_CONFIRMED.store(true, Ordering::SeqCst);

    reset_watchdog_state();

    assert!(
        !PAINT_CONFIRMED.load(Ordering::SeqCst),
        "a recreated window gets its own paint probe"
    );
}

#[test]
#[serial]
fn a_recreated_window_resets_the_reload_counter() {
    // The predecessor spent its whole budget on a blank first load. The
    // recreated window must be reloadable again, not inherit the spent count.
    WATCHDOG_RELOADS.store(RECOVERY_RELOAD_LIMIT, Ordering::SeqCst);
    assert_eq!(
        watchdog_verdict(&state(
            true,
            true,
            false,
            true,
            WATCHDOG_RELOADS.load(Ordering::SeqCst)
        )),
        WatchdogVerdict::Idle,
        "the predecessor's budget is spent"
    );

    reset_watchdog_state();

    let reloads = WATCHDOG_RELOADS.load(Ordering::SeqCst);
    assert_eq!(reloads, 0, "the recreated window starts a new budget");
    assert_eq!(
        watchdog_verdict(&state(true, true, false, true, reloads)),
        WatchdogVerdict::Reload,
        "a blank recreated window must reload again"
    );
}

#[test]
#[serial]
fn a_recreated_window_counts_from_its_own_creation() {
    *WINDOW_CREATED
        .lock()
        .expect("The creation clock lock should be held") =
        Some(Instant::now() - Duration::from_secs(100));

    reset_watchdog_state();

    assert!(
        window_age() < Duration::from_secs(1),
        "the load bound of a recreated window starts now"
    );
}

#[test]
#[serial]
fn a_painted_probe_latches_the_paint() {
    reset_watchdog_state();

    let painted = record_paint_probe(PaintProbe::Painted);

    assert!(painted, "a painted answer records a paint");
    assert!(
        PAINT_CONFIRMED.load(Ordering::SeqCst),
        "a painted answer latches the paint for the window"
    );
}

#[test]
#[serial]
fn a_missed_probe_leaves_the_paint_unconfirmed() {
    for probe in [PaintProbe::Blank, PaintProbe::Unanswered] {
        reset_watchdog_state();

        let painted = record_paint_probe(probe);

        assert!(!painted, "{probe:?} is not a paint");
        assert!(
            !PAINT_CONFIRMED.load(Ordering::SeqCst),
            "{probe:?} must not latch a paint"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_ready_reports_a_200_origin() {
    let (listener, url) = origin_url();
    answer_once(listener, b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");

    assert!(
        crate::server::origin_ready(&url).await,
        "an origin answering 200 is ready: {url}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_ready_reports_a_refused_origin() {
    let (listener, url) = origin_url();
    drop(listener);

    assert!(
        !crate::server::origin_ready(&url).await,
        "a refused origin is not ready: {url}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_ready_reports_a_non_200_origin() {
    let (listener, url) = origin_url();
    answer_once(
        listener,
        b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\n\r\n",
    );

    assert!(
        !crate::server::origin_ready(&url).await,
        "only an HTTP 200 origin is ready: {url}"
    );
}
