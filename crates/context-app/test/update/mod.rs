//! Tests for the update flow: the gating decision, the busy guard, and the
//! startup reconciliation after an install.

use std::sync::Mutex;

use super::{
    claim, release, settled_after_install, should_check, state_available, state_with, transition,
};
use crate::update::releases::{Candidate, manifest_url};
use context_core::update::{
    UpdateState, UpdateStateError, UpdateStatus, load as load_state, update_state_path,
};
use context_core::{clear_base_path, set_base_path};
use context_server::api::notifier::{ChangeNotifier, UpdateMessage};
use semver::Version;
use serial_test::serial;
use tempfile::TempDir;

#[test]
fn a_manual_check_always_runs() {
    assert!(should_check(true, true));
    assert!(
        should_check(true, false),
        "a manual check must run even while auto-update is off"
    );
}

#[test]
fn an_automatic_check_follows_the_auto_update_setting() {
    assert!(should_check(false, true));
    assert!(
        !should_check(false, false),
        "an automatic check must stay quiet while auto-update is off"
    );
}

#[test]
fn the_busy_flag_admits_one_run_at_a_time() {
    let busy = Mutex::new(false);

    assert!(claim(&busy).is_ok(), "the first run claims the flow");
    assert!(
        matches!(claim(&busy), Err(UpdateStateError::Busy)),
        "a second run must be refused while the first is in flight"
    );

    release(&busy);

    assert!(
        claim(&busy).is_ok(),
        "the flow accepts a run again once the first finished"
    );
}

#[test]
fn an_available_state_carries_the_candidate() {
    let candidate = Candidate {
        version: Version::parse("0.9.0").expect("test version parses"),
        tag: "v0.9.0".to_string(),
        manifest_url: manifest_url("v0.9.0"),
        is_prerelease: true,
        notes: Some("Release notes".to_string()),
    };

    let state = state_available(candidate);

    assert_eq!(state.status, UpdateStatus::Available);
    assert_eq!(state.candidate_version.as_deref(), Some("0.9.0"));
    assert_eq!(state.candidate_tag.as_deref(), Some("v0.9.0"));
    assert_eq!(state.candidate_is_prerelease, Some(true));
    assert_eq!(state.candidate_notes.as_deref(), Some("Release notes"));
    assert!(
        state.message.is_none() && state.updated_at.is_some(),
        "an available state carries no failure message and a timestamp"
    );
}

#[test]
fn an_idle_state_clears_the_candidate() {
    let state = state_with(UpdateStatus::Idle);

    assert_eq!(state.status, UpdateStatus::Idle);
    assert!(
        state.candidate_version.is_none()
            && state.candidate_tag.is_none()
            && state.candidate_notes.is_none()
            && state.candidate_is_prerelease.is_none()
            && state.message.is_none(),
        "an idle state carries no candidate and no message"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn a_transition_writes_the_state_and_broadcasts_the_status() {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());

    let notifier = ChangeNotifier::new();
    let mut receiver = notifier.subscribe();
    let mut state = state_with(UpdateStatus::Available);
    state.candidate_version = Some("0.9.0".to_string());

    let written = transition(&notifier, state);
    let broadcast = receiver.recv().await;
    let stored = load_state(&update_state_path());

    clear_base_path();

    assert!(written.is_ok(), "the transition writes the state file");
    let stored = stored.expect("the state file loads after the transition");
    assert_eq!(stored.status, UpdateStatus::Available);
    assert_eq!(stored.candidate_version.as_deref(), Some("0.9.0"));

    assert_eq!(
        broadcast.expect("the transition broadcasts the new status"),
        UpdateMessage::UpdateStateChanged {
            status: "available".to_string(),
            version: Some("0.9.0".to_string()),
        }
    );
}

#[test]
fn an_installing_state_whose_candidate_is_the_running_version_is_settled() {
    let state = UpdateState {
        status: UpdateStatus::Installing,
        current_version: "0.8.0".to_string(),
        candidate_version: Some("0.8.0".to_string()),
        ..UpdateState::default()
    };

    assert!(
        settled_after_install(&state),
        "the new binary is the candidate it installed"
    );
}

#[test]
fn an_installing_state_for_another_version_is_not_settled() {
    let state = UpdateState {
        status: UpdateStatus::Installing,
        current_version: "0.7.10".to_string(),
        candidate_version: Some("0.8.0".to_string()),
        ..UpdateState::default()
    };

    assert!(!settled_after_install(&state));
}

#[test]
fn an_installing_state_without_a_candidate_is_not_settled() {
    let state = UpdateState {
        status: UpdateStatus::Installing,
        current_version: "0.8.0".to_string(),
        ..UpdateState::default()
    };

    assert!(!settled_after_install(&state));
}

#[test]
fn every_other_status_is_not_settled() {
    for status in [
        UpdateStatus::Idle,
        UpdateStatus::Checking,
        UpdateStatus::Available,
        UpdateStatus::Downloading,
        UpdateStatus::Failed,
    ] {
        let state = UpdateState {
            status,
            current_version: "0.8.0".to_string(),
            candidate_version: Some("0.8.0".to_string()),
            ..UpdateState::default()
        };

        assert!(
            !settled_after_install(&state),
            "{status:?} must not reset to idle"
        );
    }
}
