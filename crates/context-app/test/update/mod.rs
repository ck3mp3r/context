//! Tests for the update flow: the gating decision, the busy guard, the startup
//! reconciliation after an install, and the restart sequence.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    CheckDecision, CheckGate, check_gate, check_target, claim, current_build_id, current_version,
    dev_build_refusal, read_state, release, restart_gate, restart_sequence, restart_target,
    settled_after_install, should_check, state_available, state_with, transition,
};
use crate::update::releases::{Candidate, manifest_url};
use context_core::settings::Settings;
use context_core::update::{UpdateState, UpdateStateError, UpdateStatus, load as load_state};
use context_server::api::notifier::{ChangeNotifier, UpdateMessage};
use semver::Version;
use tempfile::TempDir;

/// Settings the gate reads in these tests.
///
/// Every field is spelled out so the tests never reach the configuration path or
/// the base-path global.
fn gate_settings(auto_update: bool) -> Settings {
    Settings {
        host: "127.0.0.1".parse().expect("test host parses"),
        port: 3737,
        verbosity: 0,
        enable_docs: false,
        skills_dir: PathBuf::from("/tmp/update-gate-skills"),
        start_at_login: true,
        auto_update,
        update_channel: "pre-release".to_string(),
    }
}

/// Gate over a fixed set of settings, driven without an app handle.
fn gate(auto_update: bool) -> CheckGate<impl Fn() -> Settings> {
    let settings = gate_settings(auto_update);

    CheckGate::new(move || settings.clone())
}

#[cfg(debug_assertions)]
#[test]
fn a_debug_build_refuses_to_install_and_restart() {
    let refusal = dev_build_refusal().expect("a debug build must refuse install and restart");

    assert!(
        matches!(refusal, UpdateStateError::Invalid { ref message } if message.contains("development builds") && message.contains("restart")),
        "the refusal names the development build and both actions, got: {refusal:?}"
    );
}

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
async fn a_transition_writes_the_state_and_broadcasts_the_status() {
    let dir = TempDir::new().expect("Failed to create base temp dir");
    let path = dir.path().join("update.json");

    let notifier = ChangeNotifier::new();
    let mut receiver = notifier.subscribe();
    let mut state = state_with(UpdateStatus::Available);
    state.candidate_version = Some("0.9.0".to_string());

    let written = transition(&path, &notifier, state);
    let broadcast = receiver.recv().await;
    let stored = load_state(&path);

    assert!(written.is_ok(), "the transition writes the state file");
    let stored = stored.expect("the state file loads after the transition");
    assert_eq!(stored.status, UpdateStatus::Available);
    assert_eq!(stored.candidate_version.as_deref(), Some("0.9.0"));

    assert_eq!(
        broadcast.expect("the transition broadcasts the new status"),
        UpdateMessage::UpdateStateChanged {
            status: UpdateStatus::Available,
            version: Some("0.9.0".to_string()),
        }
    );
}

#[test]
fn a_missing_state_file_reads_as_the_default_state() {
    let dir = TempDir::new().expect("Failed to create base temp dir");

    assert_eq!(
        read_state(&dir.path().join("update.json")),
        UpdateState::default()
    );
}

#[test]
fn an_unparsable_state_file_reads_as_the_default_state() {
    let dir = TempDir::new().expect("Failed to create base temp dir");
    let path = dir.path().join("update.json");
    std::fs::write(&path, "{ not json").expect("the corrupt file writes");

    assert_eq!(
        read_state(&path),
        UpdateState::default(),
        "a read failure must not surface to the caller"
    );
}

#[test]
fn a_written_state_file_reads_back() {
    let dir = TempDir::new().expect("Failed to create base temp dir");
    let path = dir.path().join("update.json");
    let state = UpdateState {
        status: UpdateStatus::Available,
        candidate_version: Some("0.9.0".to_string()),
        ..UpdateState::default()
    };
    context_core::update::write(&path, &state).expect("the state file writes");

    assert_eq!(read_state(&path), state);
}

#[test]
fn a_ready_state_whose_candidate_is_the_running_version_is_settled() {
    let state = UpdateState {
        status: UpdateStatus::Ready,
        current_version: "0.9.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        ..UpdateState::default()
    };

    assert!(
        settled_after_install(&state),
        "the restarted service runs the version the state waited for"
    );
}

#[test]
fn a_ready_state_for_another_version_is_not_settled() {
    let state = UpdateState {
        status: UpdateStatus::Ready,
        current_version: "0.8.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        ..UpdateState::default()
    };

    assert!(
        !settled_after_install(&state),
        "the restarted service runs a version the state never installed"
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

#[test]
fn the_gate_runs_an_automatic_check_while_auto_update_is_on() {
    assert_eq!(
        gate(true).decide(false),
        CheckDecision::Run {
            channel: "pre-release".to_string()
        },
        "the gate hands the resolved channel to the check"
    );
}

#[test]
fn the_gate_runs_a_manual_check_while_auto_update_is_off() {
    assert_eq!(
        gate(false).decide(true),
        CheckDecision::Run {
            channel: "pre-release".to_string()
        }
    );
}

#[test]
fn the_gate_declines_an_automatic_check_while_auto_update_is_off() {
    assert_eq!(
        gate(false).decide(false),
        CheckDecision::Decline,
        "auto-update off must not claim the flow"
    );
}

/// State an install leaves behind: the swapped bundle waits for a restart.
fn ready_state() -> UpdateState {
    UpdateState {
        status: UpdateStatus::Ready,
        current_version: "0.9.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        candidate_tag: Some("v0.9.0".to_string()),
        ..UpdateState::default()
    }
}

/// State with a candidate the user has not consented to.
fn available_state() -> UpdateState {
    UpdateState {
        status: UpdateStatus::Available,
        current_version: "0.8.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        candidate_tag: Some("v0.9.0".to_string()),
        ..UpdateState::default()
    }
}

/// State file inside a temp dir this test alone owns.
fn state_file(state: &UpdateState) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("Failed to create test temp dir");
    let path = dir.path().join("update.json");
    context_core::update::write(&path, state).expect("the state file writes");

    (dir, path)
}

#[test]
fn a_ready_state_refuses_a_check() {
    let refusal = check_target(&ready_state());

    assert!(
        matches!(refusal, Err(UpdateStateError::Invalid { ref message }) if message.contains("restart")),
        "a staged bundle must refuse a check and name the restart, got: {refusal:?}"
    );
}

#[test]
fn every_other_status_opens_the_check_gate() {
    for status in [
        UpdateStatus::Idle,
        UpdateStatus::Checking,
        UpdateStatus::Available,
        UpdateStatus::Downloading,
        UpdateStatus::Installing,
        UpdateStatus::Failed,
    ] {
        let state = UpdateState {
            status,
            ..ready_state()
        };

        assert!(
            check_target(&state).is_ok(),
            "{status:?} must check exactly as before"
        );
    }
}

#[test]
fn a_check_gate_refuses_ready_and_keeps_the_staged_bundle() {
    let (_dir, path) = state_file(&ready_state());
    let busy = Mutex::new(false);
    claim(&busy).expect("the trigger claims the flow");

    let outcome = check_gate(&path, &busy);

    assert!(
        matches!(outcome, Err(UpdateStateError::Invalid { .. })),
        "a check while a staged bundle waits must answer 409, got: {outcome:?}"
    );
    assert!(
        claim(&busy).is_ok(),
        "a refused check must release the claim, so the next trigger is not Busy"
    );

    let stored = read_state(&path);
    assert_eq!(
        stored.status,
        UpdateStatus::Ready,
        "a refused check writes no state, so the pill keeps its restart offer"
    );
    assert_eq!(
        stored.candidate_version.as_deref(),
        Some("0.9.0"),
        "a refused check keeps the staged candidate"
    );
}

#[test]
fn a_check_gate_holds_the_claim_while_a_candidate_waits_for_consent() {
    let (_dir, path) = state_file(&available_state());
    let busy = Mutex::new(false);
    claim(&busy).expect("the trigger claims the flow");

    assert!(
        check_gate(&path, &busy).is_ok(),
        "an available candidate does not block a check"
    );
    assert!(
        matches!(claim(&busy), Err(UpdateStateError::Busy)),
        "the claim stays held for the run the trigger spawns"
    );
}

#[test]
fn a_ready_state_carries_a_restart_target() {
    assert!(
        restart_target(&ready_state()).is_ok(),
        "a staged bundle is the thing the restart runs"
    );
}

#[test]
fn a_mid_flight_state_is_busy() {
    for status in [
        UpdateStatus::Checking,
        UpdateStatus::Downloading,
        UpdateStatus::Installing,
    ] {
        let state = UpdateState {
            status,
            ..ready_state()
        };

        assert!(
            matches!(restart_target(&state), Err(UpdateStateError::Busy)),
            "{status:?} must report the run in flight"
        );
    }
}

#[test]
fn a_state_without_a_staged_bundle_has_no_restart_target() {
    for status in [
        UpdateStatus::Idle,
        UpdateStatus::Available,
        UpdateStatus::Failed,
    ] {
        let state = UpdateState {
            status,
            ..ready_state()
        };

        assert!(
            matches!(
                restart_target(&state),
                Err(UpdateStateError::Invalid { .. })
            ),
            "{status:?} must have nothing to restart into"
        );
    }
}

#[test]
fn a_restart_gate_refuses_a_state_without_a_staged_bundle() {
    let (_dir, path) = state_file(&available_state());
    let busy = Mutex::new(false);
    claim(&busy).expect("the trigger claims the flow");

    let outcome = restart_gate(&path, &busy);

    assert!(
        matches!(outcome, Err(UpdateStateError::Invalid { .. })),
        "a state with no staged bundle must answer 409, not a 202 that no-ops, got: {outcome:?}"
    );
    assert!(
        claim(&busy).is_ok(),
        "a refused restart must release the claim, so the next trigger is not Busy"
    );
}

#[test]
fn a_restart_gate_holds_the_claim_for_a_staged_bundle() {
    let (_dir, path) = state_file(&ready_state());
    let busy = Mutex::new(false);
    claim(&busy).expect("the trigger claims the flow");

    assert!(
        restart_gate(&path, &busy).is_ok(),
        "a staged bundle opens the gate"
    );
    assert!(
        matches!(claim(&busy), Err(UpdateStateError::Busy)),
        "the claim stays held while the sequence runs"
    );
}

#[test]
fn a_restart_spawns_the_service_then_respawns_the_gui_then_exits() {
    let (_dir, path) = state_file(&ready_state());
    let log = Mutex::new(Vec::new());
    let spawned_at = Mutex::new(None);
    let replaced = AtomicBool::new(false);
    let replaced_before_exit = AtomicBool::new(false);
    let exit_after = Mutex::new(None);

    let outcome = restart_sequence(
        &path,
        |service| {
            *spawned_at.lock().expect("spawn clock lock") = Some(std::time::Instant::now());
            log.lock()
                .expect("log lock")
                .push(format!("spawn {}", service.display()));
            Ok(())
        },
        || {
            // Stands in for ServiceState::replace_gui, which returns only after
            // every GUI of the profile is gone and the replacement is spawned.
            log.lock()
                .expect("log lock")
                .push("replace gui".to_string());
            replaced.store(true, Ordering::SeqCst);
        },
        || {
            replaced_before_exit.store(replaced.load(Ordering::SeqCst), Ordering::SeqCst);
            let elapsed = spawned_at
                .lock()
                .expect("spawn clock lock")
                .expect("the service spawn opens the sequence")
                .elapsed();
            *exit_after.lock().expect("exit clock lock") = Some(elapsed);
            log.lock().expect("log lock").push("exit".to_string());
        },
    );

    assert!(outcome.is_ok(), "the staged update restarts: {outcome:?}");

    let log = log.lock().expect("log lock").clone();
    assert_eq!(
        log.len(),
        3,
        "the restart has exactly three steps — spawn, GUI, exit — and no launchd work, got: {log:?}"
    );
    assert!(
        log[0].starts_with("spawn ") && log[0].ends_with("context-service"),
        "the sequence starts the swapped service first, got: {log:?}"
    );
    assert_eq!(
        log[1], "replace gui",
        "the GUI step replaces every running GUI of the profile before the old process exits, so the new window sees a healthy origin"
    );
    assert_eq!(
        log[2], "exit",
        "the shell's sleep 1 lets the old process release the port and the single-instance socket"
    );
    assert!(
        replaced_before_exit.load(Ordering::SeqCst),
        "the GUI step must run to completion before the exit, got: {log:?}"
    );

    let exit_after = exit_after
        .lock()
        .expect("exit clock lock")
        .expect("the exit runs last");
    assert!(
        exit_after < std::time::Duration::from_secs(1),
        "the exit must land inside the shell's 1 s head start, took {exit_after:?}"
    );

    assert_eq!(
        read_state(&path).status,
        UpdateStatus::Ready,
        "the restart rewrites no state; the new process settles it at its own startup"
    );
}

#[test]
fn a_failed_service_spawn_keeps_the_old_process_serving() {
    let (_dir, path) = state_file(&ready_state());
    let gui = AtomicBool::new(false);
    let exited = AtomicBool::new(false);

    let outcome = restart_sequence(
        &path,
        |_service| Err("no shell to restart through".to_string()),
        || gui.store(true, Ordering::SeqCst),
        || exited.store(true, Ordering::SeqCst),
    );

    assert!(
        matches!(outcome, Err(UpdateStateError::Invalid { ref message }) if message.contains("no shell")),
        "the failure names the spawn error, got: {outcome:?}"
    );
    assert!(
        !gui.load(Ordering::SeqCst),
        "a restart that cannot start the new service must not touch the GUI"
    );
    assert!(
        !exited.load(Ordering::SeqCst),
        "a restart that cannot start the new service must keep serving"
    );
    assert_eq!(
        read_state(&path).status,
        UpdateStatus::Ready,
        "ready persists, so the user can try the restart again"
    );
}

#[test]
fn a_restart_without_a_staged_bundle_runs_nothing() {
    let (_dir, path) = state_file(&available_state());
    let log = Mutex::new(Vec::new());

    let outcome = restart_sequence(
        &path,
        |_service| {
            log.lock().expect("log lock").push("spawn".to_string());
            Ok(())
        },
        || log.lock().expect("log lock").push("gui".to_string()),
        || log.lock().expect("log lock").push("exit".to_string()),
    );

    assert!(
        matches!(outcome, Err(UpdateStateError::Invalid { .. })),
        "a state without a staged bundle refuses the restart, got: {outcome:?}"
    );
    assert!(
        log.lock().expect("log lock").is_empty(),
        "a refused restart must not spawn, respawn or exit anything"
    );
}

#[test]
fn the_running_version_stays_the_numeric_core() {
    let running = Version::parse(&current_version()).expect("the running version parses");

    assert!(
        running.pre.is_empty() && running.build.is_empty(),
        "the state and the DTO report the numeric core, got: {running}"
    );
}

#[test]
fn the_build_id_is_the_short_sha_or_empty_without_a_checkout() {
    let build_id = current_build_id();
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.git/HEAD");

    if !checkout.exists() {
        assert!(
            build_id.is_empty(),
            "a build with no git checkout reports an empty id, got: {build_id:?}"
        );
        return;
    }

    assert!(
        (7..=40).contains(&build_id.len()) && build_id.chars().all(|it| it.is_ascii_hexdigit()),
        "a git build embeds the short sha, got: {build_id:?}"
    );
}
