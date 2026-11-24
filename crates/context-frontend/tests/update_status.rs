//! Pins the update phrase vocabulary.
//!
//! The wasm crate cannot host-link its own test harness, so the module is included by path and
//! compiled into this binary: no leptos runtime, no wasm, plain assertions on the exact strings.

#[path = "../src/update_status.rs"]
#[allow(dead_code)]
mod update_status;

use update_status::{update_failure, update_phrase};

#[test]
fn the_busy_phases_read_as_progress() {
    assert_eq!(
        update_phrase("checking", None).as_deref(),
        Some("Checking for updates…")
    );
    assert_eq!(
        update_phrase("downloading", None).as_deref(),
        Some("Downloading update…")
    );
    assert_eq!(
        update_phrase("installing", None).as_deref(),
        Some("Finishing the update…")
    );
}

#[test]
fn a_finished_install_reads_as_a_restart() {
    assert_eq!(
        update_phrase("ready", None).as_deref(),
        Some("Update installed — restarting…")
    );
}

#[test]
fn an_available_update_names_the_version() {
    assert_eq!(
        update_phrase("available", Some("0.8.0-8959cbd")).as_deref(),
        Some("Update 0.8.0-8959cbd available")
    );
}

#[test]
fn an_available_update_without_a_version_stays_unnamed() {
    assert_eq!(
        update_phrase("available", None).as_deref(),
        Some("Update available")
    );
    assert_eq!(
        update_phrase("available", Some("   ")).as_deref(),
        Some("Update available"),
        "a blank version is not a version"
    );
}

#[test]
fn an_idle_or_unknown_status_reads_as_nothing() {
    assert_eq!(update_phrase("idle", None), None);
    assert_eq!(
        update_phrase("something-else", Some("0.9.0")),
        None,
        "an unknown status renders no phrase"
    );
}

#[test]
fn a_failure_shows_the_recorded_detail() {
    assert_eq!(
        update_failure(Some("GitHub API returned 403")),
        "GitHub API returned 403"
    );
    assert_eq!(
        update_failure(Some("  the manifest was refused  ")),
        "the manifest was refused",
        "surrounding whitespace never reaches the banner"
    );
}

#[test]
fn a_failure_without_detail_reads_as_a_plain_failure() {
    assert_eq!(update_failure(None), "The update failed.");
    assert_eq!(
        update_failure(Some("   ")),
        "The update failed.",
        "a blank message is not a detail"
    );
}
