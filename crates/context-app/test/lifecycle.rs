//! Tests for the window lifecycle: the external-link navigation decision and
//! the open-once guard that both policy hooks share.

use super::*;
use context_core::{clear_base_path, set_base_path};
use serial_test::serial;
use tempfile::TempDir;

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
