//! Tests for utils.rs.

use crate::*;

#[test]
fn default_purge_retention_is_30_days() {
    assert_eq!(DEFAULT_PURGE_RETENTION_DAYS, 30);
}

#[test]
fn purge_cutoff_is_before_now() {
    let cutoff = purge_cutoff(DEFAULT_PURGE_RETENTION_DAYS);
    assert!(
        cutoff < current_timestamp(),
        "cutoff {} should sort before now {}",
        cutoff,
        current_timestamp()
    );
}

#[test]
fn purge_retention_days_falls_back_to_default() {
    // The environment variable is not set in the test process, so the
    // resolver returns the documented default.
    if std::env::var(PURGE_RETENTION_DAYS_ENV).is_err() {
        assert_eq!(purge_retention_days(), DEFAULT_PURGE_RETENTION_DAYS);
    }
}
