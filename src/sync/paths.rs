//! Path resolution for c5t directories.
//!
//! Provides XDG-compliant path resolution with hardcoded test mode.

use std::env;
use std::path::PathBuf;

#[cfg(debug_assertions)]
const DATA_DIR_NAME: &str = "c5t-dev";

#[cfg(not(debug_assertions))]
const DATA_DIR_NAME: &str = "c5t";

/// Get XDG-compliant data directory for c5t.
///
/// # Arguments
/// * `home_override` - Optional data home directory override
///
/// # Returns
/// Path to data directory: `{home_override or XDG_DATA_HOME or ~/.local/share}/c5t-dev/` (debug builds)
/// or `{home_override or XDG_DATA_HOME or ~/.local/share}/c5t/` (release builds)
///
/// # Panics
/// Panics if HOME environment variable is not set and no override provided.
pub fn get_data_dir(home_override: Option<PathBuf>) -> PathBuf {
    let base = DATA_DIR_NAME;

    let data_home = home_override.unwrap_or_else(|| {
        env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = env::var("HOME").expect("HOME environment variable not set");
                PathBuf::from(home).join(".local/share")
            })
    });

    data_home.join(base)
}

/// Get sync directory (data_dir/sync).
///
/// # Returns
/// Path to sync directory: `~/.local/share/c5t-dev/sync/` (debug) or `~/.local/share/c5t/sync/` (release)
pub fn get_sync_dir() -> PathBuf {
    get_data_dir(None).join("sync")
}

/// Get database file path (data_dir/context.db).
///
/// # Arguments
/// * `home_override` - Optional data home directory override
///
/// # Returns
/// Path to database file: `{home_override or XDG_DATA_HOME or ~/.local/share}/c5t-dev/context.db` (debug)
/// or `{home_override or XDG_DATA_HOME or ~/.local/share}/c5t/context.db` (release)
pub fn get_db_path(home_override: Option<PathBuf>) -> PathBuf {
    get_data_dir(home_override).join("context.db")
}
