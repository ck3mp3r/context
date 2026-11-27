//! Tests for the launcher: route argument building, route parsing, and the
//! tracked-child bookkeeping of the service state.

use super::*;

use context_core::update::{UpdateState, UpdateStatus};
use tempfile::TempDir;

#[test]
fn show_launches_without_arguments() {
    assert!(gui_launch_args(None).is_empty());
}

#[test]
fn settings_launches_with_the_route_argument() {
    assert_eq!(
        gui_launch_args(Some("/settings")),
        vec!["--open-route".to_string(), "/settings".to_string()]
    );
}

#[test]
fn route_argument_round_trips() {
    let arguments = gui_launch_args(Some("/settings"));

    assert_eq!(
        open_route(arguments),
        Some("/settings".to_string()),
        "the GUI must read back the route the service wrote"
    );
}

#[test]
fn missing_route_argument_leaves_the_gui_at_the_root() {
    assert_eq!(open_route(Vec::new()), None);
    assert_eq!(open_route(vec!["--verbose".to_string()]), None);
    assert_eq!(open_route(vec!["--open-route".to_string()]), None);
}

#[test]
fn route_must_stay_on_the_spa_origin() {
    assert_eq!(
        open_route(vec![
            "--open-route".to_string(),
            "//example.com/".to_string()
        ]),
        None,
        "a protocol-relative path would leave the origin"
    );
    assert_eq!(
        open_route(vec!["--open-route".to_string(), "settings".to_string()]),
        None,
        "a route without the leading slash is not a path"
    );
}

#[test]
fn sibling_gui_sits_next_to_this_executable() {
    let path = sibling_gui_path().expect("Sibling path should resolve");
    let executable = std::env::current_exe().expect("Current exe should resolve");
    let expected_name = if cfg!(debug_assertions) {
        "context"
    } else {
        "Context"
    };

    assert_eq!(path.file_name().expect("Sibling has a name"), expected_name);
    assert_eq!(path.parent(), executable.parent());
}

#[test]
fn empty_service_state_terminates_without_children() {
    // Quit before any GUI was launched must not panic.
    ServiceState::new().terminate_children();
}

#[test]
fn quit_signals_and_reaps_a_tracked_child() {
    let child = Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .expect("sleep should start");
    let pid = child.id();

    let state = ServiceState::new();
    state.track(child);
    state.terminate_children();

    // The child received SIGTERM and was reaped, so its pid is gone.
    assert!(
        !process_alive(pid),
        "tracked child {pid} should be terminated and reaped"
    );
}

#[test]
fn post_spawn_activation_stays_within_the_activation_window() {
    let budget = ACTIVATION_POLLS as u32 * ACTIVATION_POLL_INTERVAL;

    assert!(
        budget <= Duration::from_secs(3),
        "the post-spawn activation must wait at most 3 seconds, budget is {budget:?}"
    );
}

#[test]
fn restart_activation_shell_waits_then_opens_the_bundled_gui() {
    let command = restart_activation_command();

    assert_eq!(
        command.get_program().to_str(),
        Some("/bin/sh"),
        "the activation must run detached through a shell, not a service thread"
    );
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        vec![
            "-c".to_string(),
            format!(
                "sleep 2; exec /usr/bin/open -a {}",
                crate::update::install::shell_quote(gui_binary_name())
            )
        ],
        "the shell must outlive the service exit, then open the bundled GUI"
    );
}

#[test]
fn replace_stays_inside_the_restart_deadline() {
    let budget = REPLACE_GRACE + REPLACE_POLL_INTERVAL;

    assert!(
        budget <= REPLACE_DEADLINE,
        "the SIGTERM grace must fit in the restart deadline, budget is {budget:?}"
    );
}

#[test]
fn replace_ends_tracked_and_discovered_guis_before_it_spawns() {
    let tracked = Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .expect("the tracked stand-in should start");
    let tracked_pid = tracked.id();
    let discovered_pid = spawn_orphan_sleep();

    let state = ServiceState::new();
    state.track(tracked);

    let mut spawns = 0;
    let mut spawned_pid = None;
    let started = std::time::Instant::now();
    state.replace_gui_with(
        || vec![discovered_pid],
        || spawn_gui_stand_in(&mut spawns, &mut spawned_pid),
        || Ok(()),
    );
    let elapsed = started.elapsed();

    assert_eq!(spawns, 1, "the replace step must spawn exactly once");
    assert!(
        !process_alive(tracked_pid),
        "the tracked GUI {tracked_pid} should be terminated and reaped"
    );
    assert!(
        !process_alive(discovered_pid),
        "the discovered GUI {discovered_pid} should be terminated"
    );
    assert!(
        elapsed <= REPLACE_DEADLINE,
        "the replace step must stay inside {REPLACE_DEADLINE:?}, took {elapsed:?}"
    );

    let spawned_pid = spawned_pid.expect("the spawn should report its pid");
    assert!(
        process_alive(spawned_pid),
        "the replacement GUI {spawned_pid} should still run"
    );

    // The replacement is untracked by design: Quit ends only the tracked GUIs.
    state.terminate_children();
    assert!(
        process_alive(spawned_pid),
        "the replacement GUI {spawned_pid} must not be tracked"
    );

    kill_process(spawned_pid);
}

#[test]
fn replace_force_kills_a_gui_that_outlives_the_sigterm_grace() {
    // The shell ignores SIGTERM and then becomes the sleep itself, so one real
    // process survives the polite signal.
    let mut survivor = Command::new("/bin/sh")
        .args(["-c", "trap '' TERM; exec sleep 30"])
        .spawn()
        .expect("the survivor should start");
    let survivor_pid = survivor.id();

    let state = ServiceState::new();
    let mut spawns = 0;
    let mut spawned_pid = None;
    let started = std::time::Instant::now();
    state.replace_gui_with(
        || vec![survivor_pid],
        || spawn_gui_stand_in(&mut spawns, &mut spawned_pid),
        || Ok(()),
    );
    let elapsed = started.elapsed();

    assert_eq!(spawns, 1, "a survivor must not keep the GUI from spawning");
    survivor.wait().expect("the survivor should be reaped here");
    assert!(
        !process_alive(survivor_pid),
        "the GUI {survivor_pid} that ignored SIGTERM should be killed"
    );
    assert!(
        elapsed <= REPLACE_DEADLINE,
        "the forced kill must stay inside {REPLACE_DEADLINE:?}, took {elapsed:?}"
    );

    kill_process(spawned_pid.expect("the spawn should report its pid"));
}

#[test]
fn replace_spawns_at_once_without_a_running_gui() {
    let state = ServiceState::new();
    let mut spawns = 0;
    let mut spawned_pid = None;
    let started = std::time::Instant::now();
    state.replace_gui_with(
        Vec::new,
        || spawn_gui_stand_in(&mut spawns, &mut spawned_pid),
        || Ok(()),
    );
    let elapsed = started.elapsed();

    assert_eq!(spawns, 1, "the replacement must spawn");
    assert!(
        elapsed < REPLACE_GRACE,
        "an empty pid set must not wait for the grace, took {elapsed:?}"
    );

    kill_process(spawned_pid.expect("the spawn should report its pid"));
}

#[test]
fn an_update_restart_replaces_the_old_gui_before_it_spawns_the_replacement() {
    let old_gui_pid = spawn_orphan_sleep();

    let run = restart_with_a_real_gui(old_gui_pid);

    assert_eq!(
        run.log,
        vec!["spawn", "replace gui", "exit"],
        "the restart records one step per half — service, GUI, exit"
    );
    assert_eq!(run.spawns, 1, "the replace path spawns exactly one GUI");
    let expected_activation = format!(
        "sleep 2; exec /usr/bin/open -a {}",
        crate::update::install::shell_quote(gui_binary_name())
    );
    assert_eq!(
        run.activation_command.as_deref(),
        Some(expected_activation.as_str()),
        "the replace step must hand the activation to a detached shell"
    );
    assert!(
        !run.old_gui_survived_the_kill,
        "the old GUI {old_gui_pid} must be gone before the replacement spawns"
    );
    assert!(
        !process_alive(old_gui_pid),
        "the old GUI {old_gui_pid} must not survive the restart"
    );

    let exit_after = run.exit_after.expect("the exit closes the sequence");
    assert!(
        exit_after < Duration::from_secs(1),
        "the exit must land inside the shell's 1 s head start, took {exit_after:?}"
    );

    kill_process(
        run.spawned_pid
            .expect("the replacement should report its pid"),
    );
}

#[test]
fn an_update_restart_force_kills_an_old_gui_that_ignores_sigterm() {
    let old_gui_pid = spawn_orphan_survivor();

    let run = restart_with_a_real_gui(old_gui_pid);

    assert_eq!(
        run.log,
        vec!["spawn", "replace gui", "exit"],
        "a survivor must not change the shape of the restart"
    );
    assert_eq!(run.spawns, 1, "a survivor must not stop the replacement");
    assert!(
        !run.old_gui_survived_the_kill,
        "the old GUI {old_gui_pid} ignored SIGTERM and must be killed before the replacement spawns"
    );
    assert!(
        !process_alive(old_gui_pid),
        "the old GUI {old_gui_pid} must not survive the restart"
    );

    let exit_after = run.exit_after.expect("the exit closes the sequence");
    assert!(
        exit_after < Duration::from_secs(1),
        "the forced kill must still exit inside the shell's 1 s head start, took {exit_after:?}"
    );

    kill_process(
        run.spawned_pid
            .expect("the replacement should report its pid"),
    );
}

/// Deadline the whole replace step must stay inside.
const REPLACE_DEADLINE: Duration = Duration::from_millis(800);
/// How long the kernel may take to publish a SIGKILL to a pid probe.
const KILL_PUBLICATION_WAIT: Duration = Duration::from_millis(100);

/// What one restart run observed, in the order it happened.
#[derive(Default)]
struct RestartRun {
    log: Vec<&'static str>,
    spawns: usize,
    spawned_pid: Option<u32>,
    old_gui_survived_the_kill: bool,
    activation_command: Option<String>,
    service_spawned_at: Option<std::time::Instant>,
    exit_after: Option<Duration>,
}

impl RestartRun {
    /// Start the stand-in for the replacement GUI and record it.
    fn spawn_stand_in(&mut self) -> Result<Child, String> {
        spawn_gui_stand_in(&mut self.spawns, &mut self.spawned_pid)
    }
}

/// Drive the real restart sequence with a real old GUI and the real replace step.
///
/// The launcher's probe and spawn are seams, so the run never touches a real GUI
/// binary: the probe reports the orphan this test started, and the spawn starts a
/// `/bin/sleep` stand-in.
fn restart_with_a_real_gui(old_gui_pid: u32) -> RestartRun {
    let dir = TempDir::new().expect("Failed to create test temp dir");
    let path = dir.path().join("update.json");
    let state = UpdateState {
        status: UpdateStatus::Ready,
        current_version: "0.9.0".to_string(),
        candidate_version: Some("0.9.0".to_string()),
        candidate_tag: Some("v0.9.0".to_string()),
        ..UpdateState::default()
    };
    context_core::update::write(&path, &state).expect("the state file writes");

    let gui_state = ServiceState::new();
    let run = Mutex::new(RestartRun::default());

    let outcome = crate::update::restart_sequence(
        &path,
        |_service| {
            let mut run = run.lock().expect("run lock");
            run.service_spawned_at = Some(std::time::Instant::now());
            run.log.push("spawn");
            Ok(())
        },
        || {
            // The real replacement: end the tracked and the discovered GUIs, wait
            // for them, escalate, then spawn.
            gui_state.replace_gui_with(
                || vec![old_gui_pid],
                || {
                    let mut run = run.lock().expect("run lock");
                    run.old_gui_survived_the_kill = survives_the_kill(old_gui_pid);
                    run.spawn_stand_in()
                },
                || {
                    run.lock().expect("run lock").activation_command =
                        Some(restart_activation_script());
                    Ok(())
                },
            );
            run.lock().expect("run lock").log.push("replace gui");
        },
        || {
            let mut run = run.lock().expect("run lock");
            let elapsed = run
                .service_spawned_at
                .expect("the service spawn opens the sequence")
                .elapsed();
            run.exit_after = Some(elapsed);
            run.log.push("exit");
        },
    );

    assert!(outcome.is_ok(), "the staged update restarts: {outcome:?}");

    run.into_inner().expect("run lock")
}

/// Spawn the stand-in for the replacement GUI and record it.
fn spawn_gui_stand_in(spawns: &mut usize, spawned_pid: &mut Option<u32>) -> Result<Child, String> {
    *spawns += 1;

    let child = Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .map_err(|error| format!("the replacement stand-in should start: {error}"))?;
    *spawned_pid = Some(child.id());

    Ok(child)
}

/// Start a `/bin/sleep` whose parent exits, so nothing reaps it: the shape of a
/// GUI that a service does not track.
#[cfg(unix)]
fn spawn_orphan_sleep() -> u32 {
    let output = Command::new("/bin/sh")
        .args(["-c", "sleep 30 >/dev/null 2>&1 & echo $!"])
        .output()
        .expect("the shell should report the orphan pid");

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("the shell should print one pid")
}

/// Start a `/bin/sleep` that ignores SIGTERM and whose parent exits: the shape
/// of a GUI that only a forced kill can clear.
#[cfg(unix)]
fn spawn_orphan_survivor() -> u32 {
    let output = Command::new("/bin/sh")
        .args(["-c", "trap '' TERM; sleep 30 >/dev/null 2>&1 & echo $!"])
        .output()
        .expect("the shell should report the survivor pid");

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("the shell should print one pid")
}

/// Whether a GUI process still answers a probe after the launcher's kill.
///
/// SIGKILL stops the victim at once, but its pid can still answer `kill -0` for a
/// moment, until the parent that reaps it runs. The replace path kills and spawns
/// back to back, so this check waits that window out and reports what is left: a
/// zombie holds no window, no port and no socket.
#[cfg(unix)]
fn survives_the_kill(pid: u32) -> bool {
    let deadline = std::time::Instant::now() + KILL_PUBLICATION_WAIT;

    while process_alive(pid) {
        if std::time::Instant::now() >= deadline {
            return true;
        }
        std::thread::sleep(REPLACE_POLL_INTERVAL);
    }

    false
}

/// End a process this test started.
#[cfg(unix)]
fn kill_process(pid: u32) {
    let _ = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // Signal 0 probes for the process without touching it.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
