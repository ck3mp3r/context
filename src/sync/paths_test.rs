use crate::sync::paths::*;

#[test]
fn test_get_data_dir_contains_c5t_test() {
    // Just verify it ends with c5t-dev (debug) or c5t (release)
    let path = get_data_dir(None);
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
    let path = get_db_path(None);
    #[cfg(debug_assertions)]
    assert!(path.ends_with("c5t-dev/context.db"));
    #[cfg(not(debug_assertions))]
    assert!(path.ends_with("c5t/context.db"));
}
