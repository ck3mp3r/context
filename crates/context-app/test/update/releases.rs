//! Tests for the GitHub release candidate selection.
//!
//! Selection is pure, so every rule is pinned against the frozen release list in
//! `testdata/releases.json`. The fetch itself needs the network and is verified by
//! hand against the real API, so its branches are pinned through the pure
//! classifiers it delegates to: the content-type check, the rate-limit check, and
//! the token header.

use crate::update::releases::{
    GITHUB_REPO, GitHubRelease, auth_header, content_type_error, manifest_url, rate_limit_error,
    select_candidate,
};

/// The frozen GitHub release list this test tree owns, as GitHub returns it.
fn fixture_raw() -> String {
    let path = format!(
        "{}/test/update/testdata/releases.json",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("Failed to read {path}: {error}"))
}

/// The frozen GitHub release list this test tree owns.
fn fixture() -> Vec<GitHubRelease> {
    serde_json::from_str(&fixture_raw()).expect("the fixture parses as a release list")
}

/// An interception page 300 characters long: longer than the head an error reports.
fn canned_html() -> String {
    format!("<html><body>{}</body></html>", "x".repeat(274))
}

/// The first 120 characters of `body`, the head every content-type error carries.
fn head(body: &str) -> String {
    body.chars().take(120).collect()
}

/// One release with the given flags.
fn release(tag: &str, prerelease: bool, draft: bool) -> GitHubRelease {
    GitHubRelease {
        tag_name: tag.to_string(),
        prerelease,
        draft,
        body: None,
    }
}

fn version(raw: &str) -> semver::Version {
    semver::Version::parse(raw).expect("test version parses")
}

#[test]
fn the_fixture_parses_as_a_release_list() {
    let releases = fixture();

    assert!(
        releases.len() >= 4,
        "the fixture must cover stable, pre-release, unparseable, and draft entries"
    );
}

#[test]
fn the_stable_channel_ignores_prereleases() {
    let candidate = select_candidate(&fixture(), "stable", "0.7.0").expect("a stable release wins");

    assert_eq!(
        candidate.tag, "v0.7.10",
        "the pre-release 0.8.0 build must not win on the stable channel"
    );
    assert_eq!(candidate.version, version("0.7.10"));
    assert!(!candidate.is_prerelease);
    assert_eq!(
        candidate.notes.as_deref(),
        Some("Bug fixes for the 0.7 line.")
    );
}

#[test]
fn the_pre_release_channel_considers_every_release() {
    let candidate = select_candidate(&fixture(), "pre-release", "0.7.0")
        .expect("the pre-release build wins over the stable releases");

    assert_eq!(candidate.tag, "v0.8.0-b118f3a");
    assert_eq!(candidate.version, version("0.8.0-b118f3a"));
    assert!(candidate.is_prerelease);
}

#[test]
fn the_greatest_version_wins() {
    let releases = vec![
        release("v0.7.8", false, false),
        release("v0.7.10", false, false),
        release("v0.7.9", false, false),
    ];

    let candidate = select_candidate(&releases, "stable", "0.7.0").expect("a release wins");

    assert_eq!(
        candidate.tag, "v0.7.10",
        "selection is by version, not by list position"
    );
}

#[test]
fn an_unparseable_tag_is_ignored_without_failing() {
    let releases = vec![
        release("nightly", false, false),
        release("v0.7.10", false, false),
    ];

    let candidate =
        select_candidate(&releases, "stable", "0.7.0").expect("the tagged release wins");

    assert_eq!(candidate.tag, "v0.7.10");
}

#[test]
fn a_list_of_unparseable_tags_yields_no_candidate() {
    let releases = vec![release("nightly", false, false)];

    assert!(
        select_candidate(&releases, "pre-release", "0.7.0").is_none(),
        "a tag that is not a version is not a candidate"
    );
}

#[test]
fn a_candidate_that_is_not_newer_is_rejected() {
    for current in ["0.7.10", "v0.7.10", "0.8.0", "1.0.0"] {
        assert!(
            select_candidate(&fixture(), "stable", current).is_none(),
            "current version {current} must not be offered an update"
        );
    }
}

#[test]
fn a_pre_release_build_of_the_running_version_is_not_newer() {
    // The live repo's newest release is a pre-release build of the running
    // version. Semver sorts a pre-release below its base version (spec 11), so
    // the pre-release channel must not offer it as an update.
    let prerelease_build = version("0.8.0-b118f3a");
    let running = version("0.8.0");
    assert!(
        prerelease_build < running,
        "the fixture must model the live case: a pre-release below its base version"
    );

    assert!(
        select_candidate(&fixture(), "pre-release", "0.8.0").is_none(),
        "a pre-release build of the running version must not be offered"
    );
}

#[test]
fn a_draft_only_list_yields_no_candidate() {
    let releases = vec![release("v9.9.9", false, true)];

    assert!(
        select_candidate(&releases, "pre-release", "0.7.0").is_none(),
        "a draft release is not published yet"
    );
}

#[test]
fn manifest_url_names_the_release_asset() {
    assert_eq!(GITHUB_REPO, "ck3mp3r/context");
    assert_eq!(
        manifest_url("v0.7.10"),
        "https://github.com/ck3mp3r/context/releases/download/v0.7.10/latest.json"
    );
}

#[test]
fn an_interception_page_is_named_not_parsed() {
    let body = canned_html();
    assert_eq!(
        body.chars().count(),
        300,
        "the canned page must be longer than the reported head"
    );

    let error = content_type_error(Some("text/html; charset=utf-8"), &body)
        .expect("an HTML answer is an error");

    assert!(
        error.contains("GitHub API answered with text/html; charset=utf-8"),
        "the error names the content type, got: {error}"
    );
    assert!(
        error.contains("is intercepted or blocked"),
        "the error names the likely cause, got: {error}"
    );
    assert!(
        error.contains(&head(&body)),
        "the error carries the first response bytes, got: {error}"
    );
    assert!(
        !error.contains(&body),
        "the error must truncate the body, got: {error}"
    );
    assert!(
        !error.contains("Failed to parse"),
        "an intercepted answer must not be reported as a parse failure, got: {error}"
    );
}

#[test]
fn an_absent_content_type_is_named_none() {
    let error = content_type_error(None, "{}").expect("a missing content type is an error");

    assert!(
        error.contains("GitHub API answered with none;"),
        "the error names the missing header, got: {error}"
    );
}

#[test]
fn a_json_content_type_passes() {
    let raw = fixture_raw();

    assert_eq!(
        content_type_error(Some("application/json"), &raw),
        None,
        "the release list is JSON and must not be reported as intercepted"
    );
}

#[test]
fn an_exhausted_rate_limit_names_the_reset_time() {
    let error = rate_limit_error(Some("0"), Some("1764028800"))
        .expect("an exhausted rate limit is an error");

    assert!(
        error.starts_with("GitHub API rate limit exhausted"),
        "the error names the rate limit first, got: {error}"
    );
    assert!(
        error.contains("2025-11-25T00:00:00+00:00"),
        "the error carries the reset time in UTC, got: {error}"
    );
}

#[test]
fn an_exhausted_rate_limit_without_a_readable_reset_says_unreadable() {
    for reset in [None, Some(""), Some("soon")] {
        let error = rate_limit_error(Some("0"), reset)
            .expect("an exhausted rate limit is an error whatever the reset header says");

        assert!(
            error.contains("unreadable"),
            "reset {reset:?} must be reported as unreadable, got: {error}"
        );
    }
}

#[test]
fn an_unexhausted_rate_limit_passes() {
    assert_eq!(
        rate_limit_error(Some("57"), None),
        None,
        "a budget with requests left must not fail the fetch"
    );
    assert_eq!(
        rate_limit_error(None, None),
        None,
        "an absent rate-limit header must not fail the fetch"
    );
}

#[test]
fn an_absent_or_blank_token_sends_no_authorization() {
    assert_eq!(auth_header(None), None);
    assert_eq!(
        auth_header(Some("   ")),
        None,
        "a blank token must not send an empty bearer header"
    );
}

#[test]
fn a_token_becomes_a_bearer_header() {
    assert_eq!(auth_header(Some("ghp_1")).as_deref(), Some("Bearer ghp_1"));
    assert_eq!(
        auth_header(Some(" ghp_1 ")).as_deref(),
        Some("Bearer ghp_1"),
        "the token is trimmed before it reaches the header"
    );
}
