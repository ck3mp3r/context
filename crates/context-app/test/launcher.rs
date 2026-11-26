//! Tests for the launcher: route argument building, route parsing, and the
//! tracked-child bookkeeping of the service state.

use super::*;

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
    state.replace_gui_with(Vec::new, || {
        spawn_gui_stand_in(&mut spawns, &mut spawned_pid)
    });
    let elapsed = started.elapsed();

    assert_eq!(spawns, 1, "the replacement must spawn");
    assert!(
        elapsed < REPLACE_GRACE,
        "an empty pid set must not wait for the grace, took {elapsed:?}"
    );

    kill_process(spawned_pid.expect("the spawn should report its pid"));
}

/// Deadline the whole replace step must stay inside.
const REPLACE_DEADLINE: Duration = Duration::from_millis(800);

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
