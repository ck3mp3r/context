//! Database utility functions.

use sqlx::types::chrono::Utc;

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
