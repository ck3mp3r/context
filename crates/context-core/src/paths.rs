//! Path resolution for c5t directories.
//!
//! ## Two roots, one profile name
//!
//! Data and configuration resolve independently:
//! - **Data** follows `XDG_DATA_HOME`, else `~/.local/share`. It holds
//!   `context.db` and the sync repository.
//! - **Configuration** follows the platform config root: `~/Library/Application
//!   Support` on macOS, `%APPDATA%` on Windows, `~/.config` elsewhere. It holds
//!   `config.toml`.
//!
//! The profile name `c5t-dev` (debug) or `c5t` (release) is appended to both, so a
//! debug build never shares a database or a settings file with a release build.
//! The split stays on purpose: it isolates the development server from the
//! production one.
//!
//! ## One config location for the app, the service and the CLI
//!
//! `XDG_CONFIG_HOME` does NOT move the configuration directory. The app and
//! `context-service` start from launchd or Finder without the shell's
//! environment, while a terminal CLI inherits it; honoring the variable would
//! give the same profile two different `config.toml` files. Ignoring it makes the
//! location deterministic. The data root still honors `XDG_DATA_HOME`, because no
//! launchd process opens the database.
//!
//! ## Base path override
//!
//! A singleton override replaces both roots:
//! - **API startup**: Calls `set_base_path(<--home value>)`
//! - **Test startup**: Calls `set_base_path(<temp dir>)`
//!
//! The override always wins over the environment. [`clear_base_path`] restores
//! environment resolution.
//!
//! ## Last resort
//!
//! [`try_get_config_dir`] and [`try_get_config_path`] return `None` when no
//! environment variable names a config root. [`get_config_dir`] and
//! [`get_config_path`] cannot fail: they fall back to `{temp_dir}/{profile}/`.
//! Only callers that cannot distinguish use the infallible form; every
//! `config.toml` consumer resolves through the `try_*` lookups.

use std::env;
use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(debug_assertions)]
const DATA_DIR_NAME: &str = "c5t-dev";

#[cfg(not(debug_assertions))]
const DATA_DIR_NAME: &str = "c5t";

/// Name of the settings file inside the configuration directory.
const CONFIG_FILE_NAME: &str = "config.toml";

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
/// use context_core::set_base_path;
/// use std::path::PathBuf;
///
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

/// Get the configuration directory for c5t.
///
/// Uses the global base path singleton if set, otherwise the platform config
/// root. `XDG_CONFIG_HOME` does not participate: the location is identical for
/// the app, the service and the CLI.
///
/// # Returns
/// Path to the configuration directory:
/// - If base path set: `{base_path}/c5t-dev/` (debug) or `{base_path}/c5t/` (release)
/// - Otherwise: `{platform config root}/c5t-dev/` (debug) or `.../c5t/` (release),
///   where the platform config root is `~/Library/Application Support` on macOS,
///   `%APPDATA%` on Windows and `~/.config` on other platforms
/// - When no environment variable names a root: `{temp_dir}/c5t-dev/` (debug) or
///   `{temp_dir}/c5t/` (release)
///
/// The last resort is silent. Callers that must distinguish use
/// [`try_get_config_dir`].
pub fn get_config_dir() -> PathBuf {
    try_get_config_dir().unwrap_or_else(|| env::temp_dir().join(DATA_DIR_NAME))
}

/// Resolve the configuration directory, or `None` when no root is named.
///
/// The base path override wins. Otherwise the platform config root resolves
/// from `APPDATA` on Windows and from `HOME` elsewhere; both unset yields
/// `None`, never a panic.
pub fn try_get_config_dir() -> Option<PathBuf> {
    let base_path = BASE_PATH_OVERRIDE.lock().unwrap();

    let config_home = match base_path.as_ref() {
        Some(path) => path.clone(),
        None => default_config_home()?,
    };

    Some(config_home.join(DATA_DIR_NAME))
}

/// Get settings file path (config_dir/config.toml).
///
/// # Returns
/// Path to settings file: `{config_dir}/config.toml`, with the documented
/// temp-directory last resort when no config root resolves.
pub fn get_config_path() -> PathBuf {
    get_config_dir().join(CONFIG_FILE_NAME)
}

/// Resolve the settings file path, or `None` when no config root is named.
pub fn try_get_config_path() -> Option<PathBuf> {
    try_get_config_dir().map(|dir| dir.join(CONFIG_FILE_NAME))
}

/// Legacy settings file location: `$XDG_CONFIG_HOME/c5t[-dev]/config.toml`.
///
/// This is the one path older builds wrote when `XDG_CONFIG_HOME` was set. The
/// migration in [`crate::settings`] copies it into the native config root once.
/// Returns `None` when `XDG_CONFIG_HOME` is unset.
pub fn legacy_config_path() -> Option<PathBuf> {
    env::var("XDG_CONFIG_HOME").ok().map(|home| {
        PathBuf::from(home)
            .join(DATA_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    })
}

/// Platform text-config root, or `None` when no environment variable names it.
fn default_config_home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            return Some(PathBuf::from(appdata));
        }
    }

    let home = env::var("HOME").ok()?;

    #[cfg(target_os = "macos")]
    let config_home = PathBuf::from(home).join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let config_home = PathBuf::from(home).join(".config");

    Some(config_home)
}
