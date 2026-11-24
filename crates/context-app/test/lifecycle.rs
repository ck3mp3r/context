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
