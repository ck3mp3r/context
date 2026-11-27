//! Pins the sync indicator vocabulary and the dirty rule.
//!
//! The wasm crate cannot host-link its own test harness, so the module is included by path and
//! compiled into this binary: no leptos runtime, no wasm, plain assertions on the exact strings.

#[path = "../src/sync_status.rs"]
#[allow(dead_code)]
mod sync_status;

use sync_status::{SyncIndicator, SyncPhase, dirty_from_status, sync_title};

fn indicator(phase: SyncPhase, dirty: bool, message: Option<&str>) -> SyncIndicator {
    SyncIndicator {
        phase,
        dirty,
        message: message.map(str::to_string),
    }
}

#[test]
fn a_settled_indicator_reads_as_synced() {
    assert_eq!(
        sync_title(&indicator(SyncPhase::Idle, false, None)),
        "Synced"
    );
}

#[test]
fn a_settled_indicator_with_local_changes_reads_as_pending() {
    assert_eq!(
        sync_title(&indicator(SyncPhase::Idle, true, None)),
        "Pending changes — click to sync"
    );
}

#[test]
fn a_running_cycle_reads_as_syncing() {
    assert_eq!(
        sync_title(&indicator(SyncPhase::Syncing, false, None)),
        "Syncing…"
    );
    assert_eq!(
        sync_title(&indicator(
            SyncPhase::Syncing,
            true,
            Some("git push was refused")
        )),
        "Syncing…",
        "the phase outranks the dirty flag and any leftover message"
    );
}

#[test]
fn a_failure_names_the_detail_it_recorded() {
    assert_eq!(
        sync_title(&indicator(
            SyncPhase::Failed,
            false,
            Some("git push was refused")
        )),
        "Sync failed: git push was refused"
    );
    assert_eq!(
        sync_title(&indicator(
            SyncPhase::Failed,
            true,
            Some("  the remote was unreachable  ")
        )),
        "Sync failed: the remote was unreachable",
        "surrounding whitespace never reaches the tooltip"
    );
}

#[test]
fn a_failure_without_detail_reads_as_a_plain_failure() {
    assert_eq!(
        sync_title(&indicator(SyncPhase::Failed, false, None)),
        "Sync failed"
    );
    assert_eq!(
        sync_title(&indicator(SyncPhase::Failed, false, Some("   "))),
        "Sync failed",
        "a blank message is not a detail"
    );
}

#[test]
fn the_dirty_rule_covers_every_combination() {
    assert!(
        !dirty_from_status(7, 0, 7, true),
        "a clean tree with matching counts has nothing to sync"
    );
    assert!(
        dirty_from_status(7, 0, 7, false),
        "uncommitted git changes make the status dirty"
    );
    assert!(
        dirty_from_status(8, 0, 7, true),
        "the database holds a row the sync files do not"
    );
    assert!(
        dirty_from_status(7, 0, 8, false),
        "the sync files hold a row the database does not"
    );
    assert!(
        !dirty_from_status(3, 2, 5, true),
        "live rows plus tombstones match the exported lines, so a synced cycle reads clean"
    );
    assert!(
        dirty_from_status(3, 2, 6, true),
        "a file line appeared, so the tombstoned export no longer matches"
    );
    assert!(
        !dirty_from_status(3, 0, 3, true),
        "a payload without tombstones counts them as zero"
    );
    assert!(
        dirty_from_status(3, 2, 5, false),
        "a dirty work tree still wins when the totals match"
    );
}
