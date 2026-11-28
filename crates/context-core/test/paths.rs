use crate::*;
use serial_test::serial;
use std::env;

use tempfile::TempDir;

/// Put a variable back the way this test found it.
fn restore_env(name: &str, previous: Option<std::ffi::OsString>) {
    match previous {
        Some(value) => unsafe { env::set_var(name, value) },
        None => unsafe { env::remove_var(name) },
    }
}

#[test]
fn test_get_data_dir_contains_c5t_test() {
    // Just verify it ends with c5t-dev (debug) or c5t (release)
    let path = get_data_dir();
    #[cfg(debug_assertions)]
    assert!(path.ends_with("c5t-dev"));
    #[cfg(not(debug_assertions))]
    assert!(path.ends_with("c5t"));
}

#[test]
fn test_get_sync_dir_contains_sync() {
    // Just verify it ends with c5t-dev/sync (debug) or c5t/sync (release)
    let path = get_sync_dir();
    #[cfg(debug_assertions)]
    assert!(path.ends_with("c5t-dev/sync"));
    #[cfg(not(debug_assertions))]
    assert!(path.ends_with("c5t/sync"));
}

#[test]
fn test_get_db_path_ends_with_context_db() {
    let path = get_db_path();
    #[cfg(debug_assertions)]
    assert!(path.ends_with("c5t-dev/context.db"));
    #[cfg(not(debug_assertions))]
    assert!(path.ends_with("c5t/context.db"));
}

#[test]
#[serial]
fn test_data_root_still_honors_xdg_data_home() {
    clear_base_path();

    let data_home = TempDir::new().expect("data temp dir");
    let config_home = TempDir::new().expect("config temp dir");
    let previous_data = env::var_os("XDG_DATA_HOME");
    let previous_config = env::var_os("XDG_CONFIG_HOME");
    unsafe {
        env::set_var("XDG_DATA_HOME", data_home.path());
        env::set_var("XDG_CONFIG_HOME", config_home.path());
    }

    let db_path = get_db_path();

    restore_env("XDG_DATA_HOME", previous_data);
    restore_env("XDG_CONFIG_HOME", previous_config);

    assert!(
        db_path.starts_with(data_home.path()),
        "the database should sit under XDG_DATA_HOME, got {db_path:?}"
    );
    assert!(
        db_path.ends_with("context.db"),
        "the database file is context.db, got {db_path:?}"
    );
    assert!(
        !db_path.starts_with(config_home.path()),
        "the config root must not move the database, got {db_path:?}"
    );
}
