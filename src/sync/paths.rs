//! Path resolution for c5t directories.
//!
//! Provides XDG-compliant path resolution with hardcoded test mode.

use std::env;
use std::path::PathBuf;

/// Get XDG-compliant data directory for c5t.
///
/// HARDCODED: Always uses "c5t-test" for testing phase.
/// To switch to production, change "c5t-test" to "c5t".
///
/// # Returns
/// Path to data directory: `~/.local/share/c5t-test/`
///
/// # Panics
/// Panics if HOME environment variable is not set and XDG_DATA_HOME is also not set.
pub fn get_data_dir() -> PathBuf {
    // HARDCODED: Always use c5t-test for now
    let base = "c5t-test";

    let data_home = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").expect("HOME environment variable not set");
            PathBuf::from(home).join(".local/share")
        });

    data_home.join(base)
}

/// Get sync directory (data_dir/sync).
///
/// # Returns
/// Path to sync directory: `~/.local/share/c5t-test/sync/`
pub fn get_sync_dir() -> PathBuf {
    get_data_dir().join("sync")
}

/// Get database file path (data_dir/context.db).
///
/// # Returns
/// Path to database file: `~/.local/share/c5t-test/context.db`
pub fn get_db_path() -> PathBuf {
    get_data_dir().join("context.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_data_dir_contains_c5t_test() {
        // Just verify it ends with c5t-test (env vars are unreliable in parallel tests)
        let path = get_data_dir();
        assert!(path.ends_with("c5t-test"));
    }

    #[test]
    fn test_get_sync_dir_contains_sync() {
        // Just verify it ends with c5t-test/sync
        let path = get_sync_dir();
        assert!(path.ends_with("c5t-test/sync"));
    }

    #[test]
    fn test_get_db_path_ends_with_context_db() {
        let path = get_db_path();
        assert!(path.ends_with("c5t-test/context.db"));
    }
}
