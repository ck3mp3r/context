//! Database utility functions.

use chrono::Utc;

/// Generate an 8-character hex ID for database entities
pub fn generate_entity_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = (duration.as_secs() as u32) ^ (duration.subsec_nanos());
    format!("{:08x}", timestamp)
}

/// Get current datetime as string in SQLite format
pub fn current_timestamp() -> String {
    Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Default number of days a tombstone is kept before it is purged.
pub const DEFAULT_PURGE_RETENTION_DAYS: i64 = 30;

/// Environment variable that overrides the tombstone retention period.
pub const PURGE_RETENTION_DAYS_ENV: &str = "C5T_PURGE_RETENTION_DAYS";

/// Resolve the tombstone retention period in days.
///
/// Reads `C5T_PURGE_RETENTION_DAYS`. A missing, unparsable, or negative value
/// falls back to [`DEFAULT_PURGE_RETENTION_DAYS`].
pub fn purge_retention_days() -> i64 {
    std::env::var(PURGE_RETENTION_DAYS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|days| *days >= 0)
        .unwrap_or(DEFAULT_PURGE_RETENTION_DAYS)
}

/// Compute the purge cutoff timestamp: now minus `retention_days`.
///
/// The result uses the same SQLite format as [`current_timestamp`], so it can
/// be compared lexicographically against a `deleted_at` column.
pub fn purge_cutoff(retention_days: i64) -> String {
    (Utc::now() - chrono::Duration::days(retention_days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

//
// TIMESTAMP HANDLING POLICY
//
// For create() methods, we follow this pattern to support both:
// 1. Normal creation (generate fresh timestamps)
// 2. Sync/import scenarios (preserve original timestamps)
// 3. Empty string handling (backward compatibility)
//
// STANDARD PATTERN (REQUIRED):
//
// ```rust
// let created_at = entity.created_at
//     .clone()
//     .filter(|s| !s.is_empty())  // REQUIRED - treats empty strings as None
//     .unwrap_or_else(current_timestamp);
// ```
//
// RATIONALE:
// - Respects input timestamps when provided (essential for sync/migration)
// - Generates fresh timestamp when None (normal creation path)
// - **Filters empty strings to prevent invalid timestamps** (critical!)
// - Consistent across all repositories
//
// WHY .filter() IS REQUIRED:
// - Option<String> timestamps can be Some("") which is invalid
// - Without filter, Some("") would store empty string in database
// - This is defensive programming for a data model issue (TODO: refactor to proper timestamp types)
//
// APPLIES TO: All create() methods in all repositories
//
// NOTE: This is a code smell - ideally we'd use proper timestamp types (chrono::DateTime)
// instead of Option<String>. This is documented as technical debt for future refactoring.
//

// For create() methods, we follow this pattern to support both:
// 1. Normal creation (generate fresh timestamps)
// 2. Sync/import scenarios (preserve original timestamps)
//
// STANDARD PATTERN:
//
// ```rust
// let created_at = entity.created_at
//     .as_ref()
//     .filter(|s| !s.is_empty())  // Treat empty strings as None (backward compat)
//     .cloned()
//     .unwrap_or_else(current_timestamp);
// ```
//
// RATIONALE:
// - Respects input timestamps when provided (essential for sync/migration)
// - Generates fresh timestamp when None or empty (normal creation path)
// - Filters empty strings for backward compatibility
// - Consistent across all repositories
//
// APPLIES TO: All create() methods in repositories
//
