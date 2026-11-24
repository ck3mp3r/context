//! Tests for the consent install sequence: the guards, the phase transitions,
//! the restart command, and the fake-executor success and failure paths.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::install::{
    BundleExecutor, InstallError, install, install_target, restart_command, shell_quote,
};
use crate::update::releases::manifest_url;
use context_core::update::{UpdateState, UpdateStatus, load as load_state, write as write_state};
use tempfile::TempDir;

/// Executor that records what it was asked to swap and reports two phases.
struct FakeExecutor {
    result: Result<(), String>,
    manifest: Mutex<Option<String>>,
    phases: Mutex<Vec<String>>,
}

impl FakeExecutor {
    fn succeeding() -> Self {
        Self {
            result: Ok(()),
            manifest: Mutex::new(None),
            phases: Mutex::new(Vec::new()),
        }
    }

    fn failing(message: &str) -> Self {
        Self {
            result: Err(message.to_string()),
            ..Self::succeeding()
        }
    }

    fn manifest(&self) -> Option<String> {
        self.manifest.lock().expect("manifest lock").clone()
    }
}

impl BundleExecutor for FakeExecutor {
    fn swap(
        &self,
        manifest_url: &str,
        on_phase: impl Fn(&str) + Send + Sync,
    ) -> Result<(), String> {
        *self.manifest.lock().expect("manifest lock") = Some(manifest_url.to_string());

        for phase in ["downloading", "swapping"] {
            self.phases
                .lock()
                .expect("phases lock")
                .push(phase.to_string());
            on_phase(phase);
        }

        self.result.clone()
    }
}

/// Publisher that writes into `path` and records every status it writes.
fn publisher<'a>(
    path: &'a Path,
    statuses: &'a Mutex<Vec<UpdateStatus>>,
) -> impl Fn(UpdateState) -> Result<(), String> + 'a {
    move |state| {
        statuses.lock().expect("statuses lock").push(state.status);
        write_state(path, &state).map_err(|error| error.to_string())
    }
}

/// Available state with a candidate to install.
fn available_state() -> UpdateState {
    UpdateState {
        status: UpdateStatus::Available,
        current_version: "0.8.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        candidate_tag: Some("v0.9.0".to_string()),
        candidate_notes: Some("Release notes".to_string()),
        candidate_is_prerelease: Some(false),
        ..UpdateState::default()
    }
}

/// State file inside a temp dir this test alone owns.
fn state_file(state: &UpdateState) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("Failed to create test temp dir");
    let path = dir.path().join("update.json");
    write_state(&path, state).expect("the state file writes");

    (dir, path)
}

#[test]
fn an_available_state_names_the_manifest_to_install() {
    let target = install_target(&available_state()).expect("an available state is installable");

    assert_eq!(target, manifest_url("v0.9.0"));
}

#[test]
fn mid_flight_states_are_busy() {
    for status in [
        UpdateStatus::Checking,
        UpdateStatus::Downloading,
        UpdateStatus::Installing,
    ] {
        let state = UpdateState {
            status,
            ..available_state()
        };

        assert!(
            matches!(install_target(&state), Err(InstallError::Busy)),
            "{status:?} must report a run in flight"
        );
    }
}

#[test]
fn states_without_a_candidate_are_not_installable() {
    for status in [
        UpdateStatus::Idle,
        UpdateStatus::Ready,
        UpdateStatus::Failed,
    ] {
        let state = UpdateState {
            status,
            ..available_state()
        };

        assert!(
            matches!(install_target(&state), Err(InstallError::NotAvailable)),
            "{status:?} must have nothing to install"
        );
    }

    let untagged = UpdateState {
        candidate_tag: None,
        ..available_state()
    };
    assert!(matches!(
        install_target(&untagged),
        Err(InstallError::NotAvailable)
    ));
}

#[test]
fn a_refused_install_leaves_the_state_file_alone() {
    let (_dir, path) = state_file(&UpdateState::default());
    let statuses = Mutex::new(Vec::new());
    let exited = AtomicBool::new(false);

    let outcome = install(
        &FakeExecutor::succeeding(),
        &path,
        publisher(&path, &statuses),
        |_path| Ok(()),
        || exited.store(true, Ordering::SeqCst),
    );

    assert!(matches!(outcome, Err(InstallError::NotAvailable)));
    assert!(
        !exited.load(Ordering::SeqCst),
        "a refused install must not exit"
    );
    assert_eq!(
        load_state(&path).expect("the state file loads").status,
        UpdateStatus::Idle,
        "a refused install must not write the state"
    );
}

#[test]
fn a_successful_swap_ends_ready_and_restarts() {
    let (_dir, path) = state_file(&available_state());
    let statuses = Mutex::new(Vec::new());
    let log = Mutex::new(Vec::new());
    let executor = FakeExecutor::succeeding();

    let outcome = install(
        &executor,
        &path,
        publisher(&path, &statuses),
        |service| {
            log.lock()
                .expect("log lock")
                .push(format!("spawn {}", service.display()));
            Ok(())
        },
        || log.lock().expect("log lock").push("exit".to_string()),
    );

    assert!(outcome.is_ok(), "the swap succeeds: {outcome:?}");
    assert_eq!(
        executor.manifest().as_deref(),
        Some(manifest_url("v0.9.0").as_str()),
        "the executor swaps the candidate's manifest"
    );
    assert_eq!(
        *statuses.lock().expect("statuses lock"),
        vec![
            UpdateStatus::Downloading,
            UpdateStatus::Downloading,
            UpdateStatus::Installing,
            UpdateStatus::Ready,
        ],
        "every phase goes through the publisher"
    );

    let stored = load_state(&path).expect("the state file loads");
    assert_eq!(stored.status, UpdateStatus::Ready);
    assert_eq!(stored.candidate_version.as_deref(), Some("0.9.0"));
    assert_eq!(stored.candidate_tag.as_deref(), Some("v0.9.0"));
    assert_eq!(stored.candidate_notes.as_deref(), Some("Release notes"));
    assert_eq!(
        stored.current_version, "0.9.0",
        "the new bundle runs the candidate version, so reconciliation can settle"
    );

    let log = log.lock().expect("log lock").clone();
    assert_eq!(
        log.len(),
        2,
        "the bootstrap spawns once, then the exit runs"
    );
    assert!(log[0].starts_with("spawn ") && log[0].ends_with("context-service"));
    assert_eq!(log[1], "exit");
}

#[test]
fn a_failed_swap_ends_failed_without_restarting() {
    let (_dir, path) = state_file(&available_state());
    let statuses = Mutex::new(Vec::new());
    let log = Mutex::new(Vec::new());

    let outcome = install(
        &FakeExecutor::failing("the bundle could not be replaced"),
        &path,
        publisher(&path, &statuses),
        |_service| {
            log.lock().expect("log lock").push("spawn".to_string());
            Ok(())
        },
        || log.lock().expect("log lock").push("exit".to_string()),
    );

    assert!(matches!(outcome, Err(InstallError::Failed(_))));
    assert!(
        log.lock().expect("log lock").is_empty(),
        "a failed swap must not restart"
    );

    let stored = load_state(&path).expect("the state file loads");
    assert_eq!(stored.status, UpdateStatus::Failed);
    assert!(
        stored
            .message
            .as_deref()
            .is_some_and(|message| message.contains("the bundle could not be replaced")),
        "the failure message names the plugin error, got: {:?}",
        stored.message
    );
}

#[test]
fn a_failed_restart_does_not_exit() {
    let (_dir, path) = state_file(&available_state());
    let statuses = Mutex::new(Vec::new());
    let exited = AtomicBool::new(false);

    let outcome = install(
        &FakeExecutor::succeeding(),
        &path,
        publisher(&path, &statuses),
        |_service| Err("no shell to restart through".to_string()),
        || exited.store(true, Ordering::SeqCst),
    );

    assert!(matches!(outcome, Err(InstallError::Failed(_))));
    assert!(
        !exited.load(Ordering::SeqCst),
        "a service that cannot restart must keep serving"
    );

    let stored = load_state(&path).expect("the state file loads");
    assert_eq!(stored.status, UpdateStatus::Failed);
    assert!(
        stored
            .message
            .as_deref()
            .is_some_and(|message| message.contains("no shell")),
        "the failure message names the restart error, got: {:?}",
        stored.message
    );
}

#[test]
fn restart_command_restarts_through_a_shell() {
    let service = Path::new("/Applications/Context.app/Contents/MacOS/context-service");

    let command = restart_command(service);

    assert_eq!(
        command.get_program().to_str(),
        Some("/bin/sh"),
        "the bootstrap runs through a shell"
    );
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        vec![
            "-c".to_string(),
            "sleep 1; exec '/Applications/Context.app/Contents/MacOS/context-service'".to_string()
        ],
        "the shell waits for the old single-instance socket, then execs the new binary"
    );
}

#[test]
fn restart_command_quotes_a_path_that_carries_a_single_quote() {
    let service = Path::new("/Users/o'brien/Context.app/Contents/MacOS/context-service");

    let command = restart_command(service);

    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        vec![
            "-c".to_string(),
            "sleep 1; exec '/Users/o'\\''brien/Context.app/Contents/MacOS/context-service'"
                .to_string()
        ],
        "the quote must not end the quoted argument"
    );

    // The escaped form must hand the shell the exact path back.
    let quoted = shell_quote(&service.display().to_string());
    let output = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("printf '%s' {quoted}"))
        .output()
        .expect("the shell runs");

    assert_eq!(
        String::from_utf8(output.stdout).expect("the shell emits UTF-8"),
        service.display().to_string(),
        "the shell must read back the exact path"
    );
}
