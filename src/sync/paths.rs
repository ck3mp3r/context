//! Path resolution for c5t directories.
//!
//! Provides XDG-compliant path resolution with global base path override.
//!
//! ## Singleton Pattern for Base Path
//!
//! This module implements a SINGLETON pattern for path resolution:
//! - **API startup**: Calls `set_base_path(XDG_DATA_HOME or ~/.local/share)`
//! - **Test startup**: Calls `set_base_path(/tmp/test-xyz)`
//! - **All code**: Calls `get_data_dir()` → uses singleton base + constant (c5t-dev or c5t)
//!
//! This ensures ONE canonical function knows the base path, following SOLID principles.

use std::env;
use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(debug_assertions)]
const DATA_DIR_NAME: &str = "c5t-dev";

#[cfg(not(debug_assertions))]
const DATA_DIR_NAME: &str = "c5t";

/// Global base path override.
///
/// - Production: Set to XDG_DATA_HOME or ~/.local/share
/// - Tests: Set to temp directory
///
/// Uses Mutex instead of OnceLock so tests can set/clear as needed.
static BASE_PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set the global base path (for API startup or tests).
///
/// The final data directory will be: `{base_path}/{c5t-dev or c5t}/`
///
/// # Arguments
/// * `path` - Base path (e.g., ~/.local/share or /tmp/test-xyz)
///
/// # Example
/// ```no_run
/// use std::path::PathBuf;
/// // API startup
/// set_base_path(PathBuf::from("/home/user/.local/share"));
///
/// // Test startup
/// set_base_path(std::env::temp_dir().join("test-xyz"));
/// ```
pub fn set_base_path(path: PathBuf) {
    *BASE_PATH_OVERRIDE.lock().unwrap() = Some(path);
}

/// Clear the global base path override (for tests only).
///
/// After clearing, `get_data_dir()` will use XDG defaults.
pub fn clear_base_path() {
    *BASE_PATH_OVERRIDE.lock().unwrap() = None;
}

/// Get XDG-compliant data directory for c5t.
///
/// Uses the global base path singleton if set, otherwise falls back to XDG.
///
/// # Returns
/// Path to data directory:
/// - If base path set: `{base_path}/c5t-dev/` (debug) or `{base_path}/c5t/` (release)
/// - Otherwise: `{XDG_DATA_HOME or ~/.local/share}/c5t-dev/` (debug) or `.../c5t/` (release)
///
/// # Panics
/// Panics if HOME environment variable is not set and no base path override provided.
pub fn get_data_dir() -> PathBuf {
    let base_path = BASE_PATH_OVERRIDE.lock().unwrap();

    let data_home = if let Some(path) = base_path.as_ref() {
        path.clone()
    } else {
        env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = env::var("HOME").expect("HOME environment variable not set");
                PathBuf::from(home).join(".local/share")
            })
    };

    data_home.join(DATA_DIR_NAME)
}

/// Get sync directory (data_dir/sync).
///
/// # Returns
/// Path to sync directory: `~/.local/share/c5t-dev/sync/` (debug) or `~/.local/share/c5t/sync/` (release)
pub fn get_sync_dir() -> PathBuf {
    get_data_dir().join("sync")
}

/// Get database file path (data_dir/context.db).
///
/// # Returns
/// Path to database file: `{data_dir}/context.db`
pub fn get_db_path() -> PathBuf {
    get_data_dir().join("context.db")
}
