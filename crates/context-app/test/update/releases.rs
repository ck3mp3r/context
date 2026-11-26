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

/// One release with the given flags and no publication time.
fn release(tag: &str, prerelease: bool, draft: bool) -> GitHubRelease {
    GitHubRelease {
        tag_name: tag.to_string(),
        prerelease,
        draft,
        published_at: None,
        body: None,
    }
}

/// One published release with the given flags and RFC 3339 publication time.
fn published(tag: &str, prerelease: bool, published_at: &str) -> GitHubRelease {
    GitHubRelease {
        published_at: Some(published_at.to_string()),
        ..release(tag, prerelease, false)
    }
}

fn version(raw: &str) -> semver::Version {
    semver::Version::parse(raw).expect("test version parses")
}

#[test]
fn the_fixture_is_the_real_release_list() {
    let tags: Vec<String> = fixture().into_iter().map(|it| it.tag_name).collect();

    assert_eq!(
        tags,
        [
            "v0.8.0-340d170",
            "v0.8.0-4610dfd",
            "v0.8.0-a478046",
            "v0.8.0-8959cbd",
            "v0.8.0-83f33f6",
            "v0.7.10",
        ],
        "the fixture must stay the release list the live repository reports, newest first"
    );
}

#[test]
fn the_stable_channel_ignores_prereleases() {
    let candidate =
        select_candidate(&fixture(), "stable", "0.7.0", "").expect("a stable release wins");

    assert_eq!(
        candidate.tag, "v0.7.10",
        "the pre-release 0.8.0 build must not win on the stable channel"
    );
    assert_eq!(candidate.version, version("0.7.10"));
    assert!(!candidate.is_prerelease);
    assert!(
        candidate
            .notes
            .as_deref()
            .is_some_and(|notes| notes.starts_with("## Features")),
        "the release body is carried into the candidate, got: {:?}",
        candidate.notes
    );
}

#[test]
fn the_pre_release_channel_considers_every_release() {
    let candidate = select_candidate(&fixture(), "pre-release", "0.7.0", "")
        .expect("the pre-release build wins over the stable releases");

    assert_eq!(
        candidate.tag, "v0.8.0-340d170",
        "the newest by publication time wins, though its sha sorts lower as text"
    );
    assert_eq!(candidate.version, version("0.8.0-340d170"));
    assert!(candidate.is_prerelease);
}

#[test]
fn the_greatest_version_wins() {
    let releases = vec![
        release("v0.7.8", false, false),
        release("v0.7.10", false, false),
        release("v0.7.9", false, false),
    ];

    let candidate = select_candidate(&releases, "stable", "0.7.0", "").expect("a release wins");

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
        select_candidate(&releases, "stable", "0.7.0", "").expect("the tagged release wins");

    assert_eq!(candidate.tag, "v0.7.10");
}

#[test]
fn a_list_of_unparseable_tags_yields_no_candidate() {
    let releases = vec![release("nightly", false, false)];

    assert!(
        select_candidate(&releases, "pre-release", "0.7.0", "").is_none(),
        "a tag that is not a version is not a candidate"
    );
}

#[test]
fn a_candidate_that_is_not_newer_is_rejected() {
    for current in ["0.7.10", "v0.7.10", "0.8.0", "1.0.0"] {
        assert!(
            select_candidate(&fixture(), "stable", current, "").is_none(),
            "current version {current} must not be offered an update"
        );
    }
}

#[test]
fn the_running_build_is_not_offered_as_an_update() {
    assert!(
        select_candidate(&fixture(), "pre-release", "0.8.0", "340d170").is_none(),
        "a dev build must not be offered the release of its own branch"
    );
}

#[test]
fn a_release_binary_is_not_offered_its_own_release() {
    let candidate = select_candidate(&fixture(), "pre-release", "0.8.0-340d170", "8263a60");

    assert!(
        candidate.is_none(),
        "a release binary must not offer the release it was cut from, though the tag sha and \
         the build id differ, got: {:?}",
        candidate.map(|offered| offered.tag)
    );
}

#[test]
fn another_build_of_the_running_release_is_offered() {
    let candidate = select_candidate(&fixture(), "pre-release", "0.8.0-4610dfd", "53b4331")
        .expect("a newer build of the running release is an update");

    assert_eq!(candidate.tag, "v0.8.0-340d170");
}

#[test]
fn a_bare_version_is_not_suppressed_by_version_equality() {
    let releases = vec![published("v0.8.0-340d170", true, "2026-11-26T10:43:22Z")];

    let candidate = select_candidate(&releases, "pre-release", "0.8.0", "8263a60")
        .expect("a dev binary of the same core is offered the pre-release build");

    assert_eq!(candidate.tag, "v0.8.0-340d170");
}

#[test]
fn a_newer_build_of_the_running_core_is_offered() {
    let candidate = select_candidate(&fixture(), "pre-release", "0.8.0", "a478046")
        .expect("another build of the running version is an update");

    assert_eq!(
        candidate.tag, "v0.8.0-340d170",
        "the newest by publication time wins, though its sha sorts lower as text"
    );
}

#[test]
fn the_stable_channel_offers_nothing_newer_than_the_running_core() {
    for build_id in ["4610dfd", "a478046", ""] {
        assert!(
            select_candidate(&fixture(), "stable", "0.8.0", build_id).is_none(),
            "the only stable release, 0.7.10, is older than the running 0.8.0"
        );
    }
}

#[test]
fn an_empty_build_id_treats_an_equal_core_as_the_running_build() {
    assert!(
        select_candidate(&fixture(), "pre-release", "0.8.0", "").is_none(),
        "without an id the builds of one version cannot be told apart, so offering one would loop"
    );
}

#[test]
fn an_empty_build_id_still_accepts_a_higher_core() {
    let candidate = select_candidate(&fixture(), "pre-release", "0.7.0", "")
        .expect("a higher core is an update whatever the build id is");

    assert_eq!(candidate.tag, "v0.8.0-340d170");
}

#[test]
fn the_newest_by_publication_time_wins_whatever_the_list_order() {
    let mut releases = fixture();
    releases.reverse();

    let candidate =
        select_candidate(&releases, "pre-release", "0.7.0", "").expect("a release wins");

    assert_eq!(
        candidate.tag, "v0.8.0-340d170",
        "selection reads publication time, not list position"
    );
}

#[test]
fn an_undated_release_sorts_below_a_dated_one() {
    let releases = vec![
        release("v0.9.0-aaaaaaa", true, false),
        published("v0.9.0-bbbbbbb", true, "2026-11-25T20:59:43Z"),
    ];

    let candidate =
        select_candidate(&releases, "pre-release", "0.8.0", "4610dfd").expect("a release wins");

    assert_eq!(
        candidate.tag, "v0.9.0-bbbbbbb",
        "a release with no publication time cannot outrank a dated one"
    );
}

#[test]
fn a_stable_release_of_the_running_core_is_offered_to_a_prerelease_build() {
    let releases = vec![published("v0.8.0", false, "2026-11-26T10:00:00Z")];

    let candidate = select_candidate(&releases, "stable", "0.8.0", "4610dfd")
        .expect("the stable release of the running version is an update to it");

    assert_eq!(candidate.tag, "v0.8.0");
    assert_eq!(candidate.version, version("0.8.0"));
}

#[test]
fn a_draft_only_list_yields_no_candidate() {
    let releases = vec![release("v9.9.9", false, true)];

    assert!(
        select_candidate(&releases, "pre-release", "0.7.0", "").is_none(),
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
