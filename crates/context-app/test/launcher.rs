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

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // Signal 0 probes for the process without touching it.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
