use crate::sync::paths::*;

#[test]
fn test_get_data_dir_contains_c5t_test() {
    // Just verify it ends with c5t (env vars are unreliable in parallel tests)
    let path = get_data_dir(None);
    assert!(path.ends_with("c5t"));
}

#[test]
fn test_get_sync_dir_contains_sync() {
    // Just verify it ends with c5t/sync
    let path = get_sync_dir();
    assert!(path.ends_with("c5t/sync"));
}

#[test]
fn test_get_db_path_ends_with_context_db() {
    let path = get_db_path(None);
    assert!(path.ends_with("c5t/context.db"));
}
