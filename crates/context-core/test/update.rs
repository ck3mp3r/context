//! Tests for update.rs.

use crate::update::{
    NoUpdateFlow, UpdateFlow, UpdateState, UpdateStateError, UpdateStatus, load, update_state_path,
    write,
};
use crate::*;
use serial_test::serial;
use tempfile::TempDir;

#[cfg(debug_assertions)]
const PROFILE_DIR: &str = "c5t-dev";
#[cfg(not(debug_assertions))]
const PROFILE_DIR: &str = "c5t";

/// Update state with every field set.
fn full_state() -> UpdateState {
    UpdateState {
        status: UpdateStatus::Available,
        current_version: "0.8.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        candidate_tag: Some("v0.9.0".to_string()),
        candidate_notes: Some("Fixes and features".to_string()),
        candidate_is_prerelease: Some(false),
        message: Some("Update available".to_string()),
        updated_at: Some("2026-01-01T00:00:00Z".to_string()),
    }
}

#[test]
fn default_state_is_idle_at_the_workspace_version() {
    let state = UpdateState::default();

    assert_eq!(state.status, UpdateStatus::Idle);
    assert_eq!(
        state.current_version,
        env!("CARGO_PKG_VERSION"),
        "the default state should name the workspace version"
    );
    assert_eq!(state, UpdateState::default());
    assert!(
        state.candidate_version.is_none()
            && state.candidate_tag.is_none()
            && state.candidate_notes.is_none()
            && state.candidate_is_prerelease.is_none()
            && state.message.is_none()
            && state.updated_at.is_none(),
        "the default state should leave every optional field empty"
    );
}

#[test]
fn load_returns_defaults_when_the_file_is_missing() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("update.json");

    let state = load(&path).expect("a missing file loads as defaults");

    assert_eq!(
        state,
        UpdateState::default(),
        "a missing file should resolve to the default state"
    );
}

#[test]
fn write_then_load_roundtrips_every_field() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("update.json");
    let state = full_state();

    write(&path, &state).expect("write the update state");
    let loaded = load(&path).expect("load returns the written state");

    assert_eq!(loaded, state, "roundtrip should preserve every field");
}

#[test]
fn write_creates_the_parent_directory() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("nested/profile/update.json");

    write(&path, &UpdateState::default()).expect("write creates parent directories");

    assert!(path.exists(), "write should create {path:?}");
}

#[test]
fn write_reports_io_errors_with_the_path() {
    let dir = TempDir::new().expect("test temp dir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "not a directory").expect("create the blocking file");
    let path = blocker.join("update.json");

    match write(&path, &UpdateState::default()) {
        Err(UpdateStateError::Io { message }) => assert!(
            message.contains("blocker"),
            "io message {message:?} should name the failing path"
        ),
        other => panic!("expected an io error, got {other:?}"),
    }
}

#[test]
fn load_reports_invalid_json_with_the_path() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("update.json");
    std::fs::write(&path, "{ not json\n").expect("write invalid json");

    match load(&path) {
        Err(UpdateStateError::Invalid { message }) => assert!(
            message.contains("update.json"),
            "invalid message {message:?} should name the failing path"
        ),
        other => panic!("expected an invalid error, got {other:?}"),
    }
}

#[test]
fn load_falls_back_to_idle_when_the_status_key_is_absent() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("update.json");
    std::fs::write(&path, r#"{"current_version":"0.8.0"}"#).expect("write minimal json");

    let state = load(&path).expect("a state without a status loads");

    assert_eq!(state.status, UpdateStatus::Idle);
}

#[test]
fn serialized_default_state_is_idle_and_skips_empty_fields() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("update.json");

    write(&path, &UpdateState::default()).expect("write the default state");
    let text = std::fs::read_to_string(&path).expect("read back the state file");

    assert!(
        text.contains("\"status\":\"idle\""),
        "serialized state {text:?} should carry an idle status"
    );
    for absent in [
        "candidate_version",
        "candidate_tag",
        "candidate_notes",
        "candidate_is_prerelease",
        "message",
        "updated_at",
    ] {
        assert!(
            !text.contains(absent),
            "serialized state {text:?} should skip {absent}"
        );
    }
}

#[test]
#[serial]
fn update_state_path_sits_in_the_profile_config_dir() {
    let base = TempDir::new().expect("test temp dir");
    set_base_path(base.path().to_path_buf());

    let path = update_state_path();

    clear_base_path();

    assert_eq!(
        path,
        base.path().join(PROFILE_DIR).join("update.json"),
        "state path should sit inside the profile config dir"
    );
}

#[test]
fn no_update_flow_reports_unavailable_from_check_and_install() {
    let flow = NoUpdateFlow;

    match flow.check(true) {
        Err(UpdateStateError::Unavailable) => {}
        other => panic!("expected Unavailable from check, got {other:?}"),
    }
    match flow.install() {
        Err(UpdateStateError::Unavailable) => {}
        other => panic!("expected Unavailable from install, got {other:?}"),
    }
}
